use htmlescape::decode_html;
use std::time::Duration;
use itertools::Itertools;
use regex::Regex;
use failure::Error;
use reqwest::{Client, header, RedirectPolicy, Response};
use cookie::Cookie;
use std::io::Read;
use image::{gif, jpeg, png, ImageDecoder};
use mime::{Mime, IMAGE, TEXT, HTML};
use humansize::{FileSize, file_size_opts as options};

use super::config::Rtd;
use super::buildinfo;
use super::sqlite::{Database, UrlError};

const DL_BYTES: u64 = 100 * 1024; // 100kB

lazy_static! {
    static ref USER_AGENT: String = format!(
        "Mozilla/5.0 url-bot-rs/{}", buildinfo::PKG_VERSION
    );
}

pub struct RequestParams {
    pub user_agent: String,
    pub timeout_s: u64,
    pub redirect_limit: u8,
    pub accept_lang: String
}

impl Default for RequestParams {
    fn default() -> RequestParams {
        RequestParams {
            user_agent: USER_AGENT.to_string(),
            timeout_s: 10,
            redirect_limit: 10,
            accept_lang: "en".to_string(),
        }
    }
}

#[derive(Default)]
pub struct Session {
    pub url: String,
    pub cookies: Vec<String>,
    pub request_count: u8,
    pub params: RequestParams,
}

impl Session {
    pub fn new() -> Session {
        Session::default()
    }

    pub fn accept_lang(&mut self, accept_lang: &str) -> &mut Session {
        self.params.accept_lang = accept_lang.to_string();
        self
    }

    /// Make a request attempting to conform to RFC 6265
    /// https://tools.ietf.org/html/rfc6265
    pub fn request(&mut self, url: &str) -> Result<Response, Error> {
        // follow only one redirection
        let redirect = RedirectPolicy::custom(|attempt| {
            if attempt.previous().len() == 1 {
                attempt.stop()
            } else {
                attempt.follow()
            }
        });

        let client = Client::builder()
            .gzip(false)
            .redirect(redirect)
            .timeout(Duration::from_secs(self.params.timeout_s))
            .build()?;

        self.url = url.to_string();

        loop {
            // generate cookie header
            let cookie_string: String = self.cookies
                .iter()
                .map(|s| s.parse::<Cookie>().ok())
                .flatten()
                .map(|c| format!("{}={}", c.name(), c.value()))
                .intersperse("; ".to_string())
                .collect();

            // set request headers and make request
            let resp = client.get(&self.url)
                .header(header::COOKIE, cookie_string)
                .header(header::USER_AGENT, self.params.user_agent.as_str())
                .header(header::ACCEPT_LANGUAGE, self.params.accept_lang.as_str())
                .header(header::ACCEPT_ENCODING, "identity")
                .send()?;

            debug!("[{}] <{}> → [{:?} {}]",
                self.request_count, self.url, resp.version(), resp.status());

            if resp.status().is_redirection() {
                // get new cookies from response headers
                let mut new_cookies: Vec<String> = resp.headers()
                    .get_all(header::SET_COOKIE)
                    .iter()
                    .map(|c| c.to_str().ok().and_then(|s| s.parse().ok()))
                    .flatten()
                    .filter(|c| !self.cookies.contains(c))
                    .take(32) // max 32 new cookies per request
                    .collect();

                // debug print cookie information
                if !new_cookies.is_empty() {
                    trace!("Received cookies:");
                    new_cookies
                        .iter()
                        .map(|s| s.parse::<Cookie>().ok())
                        .flatten()
                        .for_each(|c| trace!("{} = {}", c.name(), c.value()));
                    debug!("added {} cookies", new_cookies.len());
                };

                // add cookies to session
                self.cookies.append(&mut new_cookies);

                // get redirection location
                let redirected_url = resp.headers().get(header::LOCATION)
                    .and_then(|u| u.to_str().ok())
                    .and_then(|u| u.parse::<String>().ok());

                match redirected_url {
                    Some(url) => self.url = url,
                    None => bail!("Can't get redirection URL"),
                };

                // limit the number of redirections
                self.request_count += 1;
                if self.request_count > self.params.redirect_limit {
                    bail!("Too many redirects, max {}",
                        self.params.redirect_limit);
                }
            }

            else if resp.status().is_success() {
                debug!("total redirections: {}, total cookies: {}",
                    self.request_count,
                    self.cookies.len());
                return Ok(resp);
            }

            else {
                let r = resp.error_for_status()?;
                bail!("Unhandled request status: {}", r.status());
            }
        }
    }
}

fn log_error(db: &Database, url: &str, err: &Error, resp: &Response) -> Result<(), Error> {
    let err = UrlError {
        url,
        error: &format!("{:?}", err),
        headers: &format!("{:#?}", resp.headers()),
        status: &format!("{:?}", resp.status()),
    };
    db.log_error(&err)?;
    Ok(())
}

pub fn resolve_url(url: &str, rtd: &Rtd, db: &Database) -> Result<String, Error> {
    let mut resp = Session::new()
        .accept_lang(&rtd.conf.params.accept_lang)
        .request(url)?;

    match get_title(&mut resp, rtd, false) {
        Ok(title) => Ok(title),
        Err(err) => {
            match log_error(&db, url, &err, &resp) {
                Ok(_) => info!("added entry for <{}> to error database", url),
                Err(e) => error!("database error: {}", e)
            };
            Err(err)
        }
    }
}

pub fn get_title(resp: &mut Response, rtd: &Rtd, dump: bool) -> Result<String, Error> {
    // get content type
    let content_type = resp.headers().get(header::CONTENT_TYPE)
        .and_then(|typ| typ.to_str().ok())
        .and_then(|typ| typ.parse::<Mime>().ok());

    // get content length and human-readable size
    let len = resp.content_length().unwrap_or(0);
    let size = len.file_size(options::CONVENTIONAL).unwrap_or_default();

    // debug printing
    trace!("Response headers:");
    resp.headers().iter().for_each(|(k, v)| {
        trace!("[{}] {}", k, v.to_str().unwrap());
    });

    // calculate download size based on the response's MIME type
    let bytes = content_type.clone()
        .and_then(|ct| {
            match (ct.type_(), ct.subtype()) {
                (IMAGE, _) => Some(10 * 1024 * 1024), // 10MB
                _ => None
            }})
        .unwrap_or(DL_BYTES);

    // download body
    let mut body = Vec::new();
    resp.take(bytes).read_to_end(&mut body)?;
    let contents = String::from_utf8_lossy(&body);

    // print downloaded body
    if dump { println!("{}", contents); }

    // get title or metadata
    let title = match content_type {
        None => parse_title(&contents),
        Some(mime) => {
            match (mime.type_(), mime.subtype()) {
                (TEXT, HTML) => parse_title(&contents),
                (IMAGE, _) => parse_title(&contents)
                    .or_else(|| get_image_metadata(&rtd, &body))
                    .or_else(|| get_mime(&rtd, &mime, &size)),
                _ => parse_title(&contents)
                    .or_else(|| get_mime(&rtd, &mime, &size)),
            }
        },
    }.ok_or_else(|| format_err!("failed to parse title"))?;

    Ok(title)
}

fn get_mime(rtd: &Rtd, mime: &Mime, size: &str) -> Option<String> {
    if rtd.conf.features.report_mime {
        Some(format!("{} {}", mime, size.replace(" ", "")))
    } else {
        None
    }
}

fn get_image_metadata(rtd: &Rtd, body: &[u8]) -> Option<String> {
    if !rtd.conf.features.report_metadata {
        None
    } else if let Ok((w, h)) = jpeg::JPEGDecoder::new(body).dimensions() {
        Some(format!("image/jpeg {}×{}", w, h))
    } else if let Ok((w, h)) = png::PNGDecoder::new(body).dimensions() {
        Some(format!("image/png {}×{}", w, h))
    } else if let Ok((w, h)) = gif::Decoder::new(body).dimensions() {
        Some(format!("image/gif {}×{}", w, h))
    } else {
        None
    }
}

fn parse_title(page_contents: &str) -> Option<String> {
    lazy_static! {
        static ref RE: Regex = Regex::new("<(?i:title).*?>((.|\n)*?)</(?i:title)>").unwrap();
    }
    let title_enc = RE.captures(page_contents)?.get(1)?.as_str();
    let title_dec = decode_html(title_enc).ok()?;

    // make any multi-line title string into a single line,
    // trim leading and trailing whitespace
    let title_one_line = title_dec
        .trim()
        .lines()
        .map(|line| line.trim())
        .join(" ");

    if title_one_line.is_empty() {
        return None;
    }

    Some(title_one_line)
}

#[cfg(test)]
mod tests {
    extern crate tiny_http;

    use super::*;
    use std::fs::File;
    use std::path::{Path, PathBuf};
    use std::{thread, time};
    use self::tiny_http::{Response, Header};
    use std::sync::mpsc;

    #[test]
    fn resolve_urls() {
        let rtd: Rtd = Rtd::default();
        let db = Database::open_in_memory().unwrap();
        resolve_url("https://youtube.com",  &rtd, &db).unwrap();
        resolve_url("https://google.co.uk", &rtd, &db).unwrap();
    }

    #[test]
    fn parse_titles() {
        assert_eq!(None, parse_title(""));
        assert_eq!(None, parse_title("    "));
        assert_eq!(None, parse_title("<title></title>"));
        assert_eq!(None, parse_title("<title>    </title>"));
        assert_eq!(None, parse_title("<TITLE>    </TITLE>"));
        assert_eq!(
            None,
            parse_title("floofynips, not a real webpage")
        );
        assert_eq!(
            Some(String::from("title caps")),
            parse_title("<TITLE>title caps</TITLE>")
        );
        assert_eq!(
            Some(String::from("title mixed caps")),
            parse_title("<TiTLe>title mixed caps</tItLE>")
        );
        assert_eq!(
            Some(String::from("cheese is nice")),
            parse_title("<title>cheese is nice</title>")
        );
        assert_eq!(
            Some(String::from("squanch")),
            parse_title("<title>     squanch</title>")
        );
        assert_eq!(
            Some(String::from("squanch")),
            parse_title("<title>squanch     </title>")
        );
        assert_eq!(
            Some(String::from("squanch")),
            parse_title("<title>\nsquanch</title>")
        );
        assert_eq!(
            Some(String::from("squanch")),
            parse_title("<title>\n  \n  squanch</title>")
        );
        assert_eq!(
            Some(String::from("we like the moon")),
            parse_title("<title>\n  \n  we like the moon</title>")
        );
        assert_eq!(
            Some(String::from("&hello123&<>''~")),
            parse_title("<title>&amp;hello123&amp;&lt;&gt;''~</title>")
        );
        assert_eq!(
            Some(String::from("CVE - CVE-2018-11235")),
            parse_title("<title>CVE -\nCVE-2018-11235\n</title>")
        );
        assert_eq!(
            Some(String::from("added properties")),
            parse_title("<title id=\"pageTitle\">added properties</title>")
        );
        assert_eq!(
            Some(String::from("\u{2665}")),
            parse_title("<title>\u{2665}</title>")
        );
    }

    #[test]
    fn get_metadata_from_local_images() {
        for test in vec!(
            ("./test/img/test.png", "image/png 800×400"),
            ("./test/img/test.jpg", "image/jpeg 400×200"),
            ("./test/img/test.gif", "image/gif 1920×1080")
        ) {
            get_local_image_metadata(test.0, test.1);
        }
    }

    fn get_local_image_metadata(file: impl AsRef<Path>, result: &str) {
        let mut rtd: Rtd = Rtd::default();

        let mut body = Vec::new();
        let f = File::open(file).unwrap();
        f.take(100 * 1024).read_to_end(&mut body).unwrap();

        rtd.conf.features.report_metadata = true;
        assert_eq!(
            Some(String::from(result)),
            get_image_metadata(&rtd, &body)
        );

        rtd.conf.features.report_metadata = false;
        assert_eq!(
            None,
            get_image_metadata(&rtd, &body)
        );
    }

    #[test]
    fn resolve_locally_served_files() {
        let mut rtd: Rtd = Rtd::default();

        // metadata and mime disabled
        rtd.conf.features.report_metadata = false;
        rtd.conf.features.report_mime = false;

        for t in vec!(
            "./test/img/test.gif",
            "./test/other/test.txt",
            "./test/other/test.pdf",
        ) {
            assert!(serve_resolve(PathBuf::from(t), &rtd).is_err());
        }

        // metadata and mime enabled
        rtd.conf.features.report_metadata = true;
        rtd.conf.features.report_mime = true;

        for t in vec!(
            ("./test/img/test.png", "image/png 800×400"),
            ("./test/img/test.jpg", "image/jpeg 400×200"),
            ("./test/img/test.gif", "image/gif 1920×1080"),
            ("./test/html/basic.html", "basic"),
            ("./test/other/test.txt", "text/plain; charset=utf8 16B"),
            ("./test/other/test.pdf", "application/pdf 1.31KB"),
        ) {
            assert_eq!(
                serve_resolve(PathBuf::from(t.0), &rtd).unwrap(),
                String::from(t.1)
            )
        }
    }

    fn get_ctype(path: &Path) -> &'static str {
        let extension = match path.extension() {
            None => return "text/plain",
            Some(e) => e
        };
        match extension.to_str().unwrap() {
            "gif" => "image/gif",
            "jpg" => "image/jpeg",
            "jpeg" => "image/jpeg",
            "png" => "image/png",
            "pdf" => "application/pdf",
            "svg" => "image/svg+xml",
            "html" => "text/html; charset=utf8",
            "txt" => "text/plain; charset=utf8",
            _ => "text/plain; charset=utf8"
        }
    }

    // Spin up a local http server, and resolve the url served
    fn serve_resolve(path: PathBuf, rtd: &Rtd) -> Result<String, Error> {
        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http("0.0.0.0:28482").unwrap();
            loop {
                let rq = server.recv().unwrap();
                if rq.url() == "/test" {
                    let resp = Response::from_file(File::open(&path).unwrap())
                        .with_header(
                            tiny_http::Header {
                                field: "Content-Type".parse().unwrap(),
                                value: get_ctype(&path).parse().unwrap(),
                            }
                        );
                    rq.respond(resp).unwrap();
                    break;
                }
            }
        });

        thread::sleep(time::Duration::from_millis(100));
        let db = Database::open_in_memory().unwrap();
        let res = resolve_url("http://0.0.0.0:28482/test", &rtd, &db);
        server_thread.join().unwrap();
        res
    }

    // Spin up a local http server, extract and verify request headers in the
    // request we make.
    //
    // Use tiny_http for this instead of hyper, to avoid using the same library
    // for both the request and the server, which could potentially mask bugs
    // in `hyper`.
    #[test]
    fn verify_request_headers() {
        let expected_headers = [
            Header::from_bytes("user-agent",
                format!("Mozilla/5.0 url-bot-rs/{}", buildinfo::PKG_VERSION)
            ).unwrap(),
            Header::from_bytes("accept", "*/*").unwrap(),
            Header::from_bytes("cookie", "").unwrap(),
            Header::from_bytes("accept-language", "en").unwrap(),
            Header::from_bytes("accept-encoding", "identity").unwrap(),
            Header::from_bytes("host", "0.0.0.0:28282").unwrap(),
        ];

        let (tx, rx) = mpsc::channel();
        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http("0.0.0.0:28282").unwrap();
            loop {
                let rq = server.recv().unwrap();
                if rq.url() == "/test" {
                    // send headers through mpsc channel
                    tx.send(rq.headers().to_owned()).unwrap();
                    // respond with some content
                    let path = Path::new("./test/html/basic.html");
                    let resp = Response::from_file(File::open(path).unwrap());
                    rq.respond(resp).unwrap();
                    break;
                }
            }
        });

        thread::sleep(time::Duration::from_millis(100));
        let db = Database::open_in_memory().unwrap();
        resolve_url("http://0.0.0.0:28282/test", &Rtd::default(), &db).unwrap();
        let request_headers = rx.recv().unwrap();

        println!("Headers in request:\n{:?}", request_headers);
        println!("Headers expected:\n{:?}", expected_headers);

        let headers_match = expected_headers
            .iter()
            .zip(request_headers.iter())
            .all(|(a, b)| {
                a.field == b.field && a.value == b.value
            });

        assert!(headers_match);
        server_thread.join().unwrap();
    }
}
