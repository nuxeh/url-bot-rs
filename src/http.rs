use std::time::Duration;
use itertools::Itertools;
use failure::Error;
use reqwest::{Client, header, RedirectPolicy, Response, Url};
use cookie::Cookie;
use std::io::Read;
use mime::{Mime, IMAGE, TEXT, HTML};
use humansize::{FileSize, file_size_opts as options};
use toml;

use super::config::Rtd;
use super::buildinfo;
use super::sqlite::{Database, UrlError, ErrorInfo};
use super::title::{parse_title, get_mime, get_image_metadata};

const CHUNK_BYTES: u64 = 100 * 1024; // 100kB
const CHUNKS_MAX: u64 = 10; // 1000kB

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
                    .and_then(|u| u.parse::<Url>().ok());

                let redirected_string = resp.headers().get(header::LOCATION)
                    .and_then(|u| u.to_str().ok())
                    .and_then(|u| u.parse::<String>().ok());

                let current_url = self.url.parse::<Url>().ok();

                let r = match (redirected_url, redirected_string, current_url) {
                    (None, Some(s), Some(u)) => u.join(&s).ok(),
                    (Some(u), _, _) => Some(u),
                    _ => None,
                };

                match r {
                    Some(url) => self.url = url.as_str().to_string(),
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

fn log_error(rtd: &Rtd, db: &Database, url: &str, err: &Error, resp: &Response) {
    if !rtd.conf.features.history { return; };

    let mut e = ErrorInfo::default();
    e.error = format!("{:?}", err);
    e.status = resp.status().as_u16();
    e.reason = resp.status().canonical_reason().unwrap_or("UNKNOWN");
    for (k, v) in resp.headers().iter() {
        e.headers.insert(k.as_str(), v.to_str().unwrap_or("ERROR"));
    };

    let err = UrlError {
        url,
        error_info: &toml::ser::to_string(&e).unwrap(),
    };

    info!("adding error record to database");

    db.log_error(&err).unwrap_or_else(|e| {
        error!("database error: {}", e);
    });
}

pub fn resolve_url(url: &str, rtd: &Rtd, db: &Database) -> Result<String, Error> {
    let mut resp = Session::new()
        .accept_lang(&rtd.conf.params.accept_lang)
        .request(url)?;

    match get_title(&mut resp, rtd, false) {
        Ok(title) => Ok(title),
        Err(err) => {
            log_error(&rtd, &db, url, &err, &resp);
            Err(err)
        },
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

    // vector to hold page content, which is progressively built from chunks of
    // downloaded data until a title is found (up to CHUNKS_MAX chunks)
    let mut body = Vec::new();

    for i in 1..=CHUNKS_MAX {
        // download a chunk
        let mut chunk = Vec::new();
        resp.take(CHUNK_BYTES).read_to_end(&mut chunk)?;

        // print downloaded chunk
        if dump { print!("{}", String::from_utf8_lossy(&chunk)); }

        // append to downloaded content (move)
        body.append(&mut chunk);

        // get title or metadata
        let contents = String::from_utf8_lossy(&body);
        let title = match content_type.clone() {
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
        };

        match title {
            Some(t) => {
                trace!("title found in {} chunks ({} B)", i, i * CHUNK_BYTES);
                return Ok(t)
            },
            None => continue,
        }
    }

    bail!(format!("{}: failed to parse title", resp.url()));
}

#[cfg(test)]
mod tests {
    extern crate tiny_http;

    use super::*;
    use std::fs::File;
    use std::path::{Path, PathBuf};
    use std::thread;
    use self::tiny_http::{Response, Header};
    use std::sync::mpsc;

    #[test]
    #[ignore]
    fn resolve_urls() {
        let rtd: Rtd = Rtd::default();
        let db = Database::open_in_memory().unwrap();
        resolve_url("https://youtube.com",  &rtd, &db).unwrap();
        resolve_url("https://google.co.uk", &rtd, &db).unwrap();
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
            }
        });

        thread::sleep(Duration::from_millis(100));
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
            Header::from_bytes("cookie", "").unwrap(),
            Header::from_bytes("user-agent",
                format!("Mozilla/5.0 url-bot-rs/{}", buildinfo::PKG_VERSION)
            ).unwrap(),
            Header::from_bytes("accept-language", "en").unwrap(),
            Header::from_bytes("accept-encoding", "identity").unwrap(),
            Header::from_bytes("accept", "*/*").unwrap(),
            Header::from_bytes("host", "0.0.0.0:28282").unwrap(),
        ];

        let (tx, rx) = mpsc::channel();
        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http("0.0.0.0:28282").unwrap();
            let rq = server.recv().unwrap();
            if rq.url() == "/test" {
                // send headers through mpsc channel
                tx.send(rq.headers().to_owned()).unwrap();

                // respond with some content
                let path = Path::new("./test/html/basic.html");
                let resp = Response::from_file(File::open(path).unwrap());
                rq.respond(resp).unwrap();
            }
        });

        thread::sleep(Duration::from_millis(100));
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

    #[test]
    fn redirect_limit() {
        redirect_limit_with_status(301); // 301 Moved Permanently https://http.cat/301
        redirect_limit_with_status(302); // 302 Found https://http.cat/302
        redirect_limit_with_status(307); // 307 Temporary Redirect https://http.cat/307
    }

    fn redirect_limit_with_status(status: u16) {
        redirect_limit_n(1, status).unwrap();
        redirect_limit_n(10, status).unwrap();
        assert!(redirect_limit_n(11, status).is_err());
    }

    fn redirect_limit_n(n: u8, status: u16) -> Result<String, Error> {
        let bind = "0.0.0.0:28284";
        let url = format!("http://{}/rlim", bind);
        let url_bytes = url.clone().into_bytes();
        let header = Header::from_bytes("location", url_bytes.clone()).unwrap();
        let db = Database::open_in_memory().unwrap();
        let timeout = Duration::from_millis(200);

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http(bind).unwrap();

            // send redirections
            for _ in 0..n {
                let rq = server.recv().unwrap();
                if rq.url() == "/rlim" {
                    let resp = Response::from_string("")
                        .with_status_code(status)
                        .with_header(header.clone());
                    rq.respond(resp).unwrap();
                }
            }

            // send success. if the resolve function errors, it will send no
            // more requests, so time out
            if let Ok(Some(rq)) = server.recv_timeout(timeout) {
                let resp = Response::from_string("<title>hello<title>")
                    .with_status_code(200);
                rq.respond(resp).unwrap();
            }
        });

        thread::sleep(Duration::from_millis(100));
        let res = resolve_url(&url, &Rtd::default(), &db);
        server_thread.join().unwrap();
        res
    }

    #[test]
    fn redirect_absolute_location() {
        let bind = "0.0.0.0:28280";
        let url = format!("http://{}/r_abs", bind);
        let url2 = format!("http://{}/r_abs_r", bind);
        let url2_bytes = url2.clone().into_bytes();
        let h_loc = Header::from_bytes("location", url2_bytes.clone()).unwrap();
        let db = Database::open_in_memory().unwrap();

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http(bind).unwrap();
            for _ in 0..2 {
                let rq = server.recv().unwrap();

                match rq.url() {
                    // redirection
                    "/r_abs" => {
                        let resp = Response::from_string("")
                            .with_status_code(301)
                            .with_header(h_loc.clone());
                        rq.respond(resp).unwrap();
                    },
                    // response
                    "/r_abs_r" => {
                        let resp = Response::from_string("<title>hello</title>");
                        rq.respond(resp).unwrap();
                    },
                    _ => (),
                }
            }
        });

        thread::sleep(Duration::from_millis(100));
        resolve_url(&url, &Rtd::default(), &db).unwrap();
        server_thread.join().unwrap();
    }

    #[test]
    fn redirect_relative_location() {
        let bind = "0.0.0.0:28278";
        let url = format!("http://{}/r_rel", bind);
        let h_loc = Header::from_bytes("location", "/r_rel_r").unwrap();
        let db = Database::open_in_memory().unwrap();

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http(bind).unwrap();
            for _ in 0..2 {
                let rq = server.recv().unwrap();

                match rq.url() {
                    // redirection
                    "/r_rel" => {
                        let resp = Response::from_string("")
                            .with_status_code(301)
                            .with_header(h_loc.clone());
                        rq.respond(resp).unwrap();
                    },
                    // response
                    "/r_rel_r" => {
                        let resp = Response::from_string("<title>hello</title>");
                        rq.respond(resp).unwrap();
                    },
                    _ => (),
                }
            }
        });

        thread::sleep(Duration::from_millis(100));
        resolve_url(&url, &Rtd::default(), &db).unwrap();
        server_thread.join().unwrap();
    }

    #[test]
    fn redirect_no_redirection_location_provided() {
        let bind = "0.0.0.0:28288";
        let url = format!("http://{}/rerr", bind);
        let db = Database::open_in_memory().unwrap();

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http(bind).unwrap();
            let rq = server.recv().unwrap();
            if rq.url() == "/rerr" {
                let resp = Response::from_string("<title>hello</title>")
                    .with_status_code(301);
                rq.respond(resp).unwrap();
            }
        });

        thread::sleep(Duration::from_millis(100));
        assert!(resolve_url(&url, &Rtd::default(), &db).is_err());
        server_thread.join().unwrap();
    }

    fn headers_contains(header: &Header, headers: &[Header]) -> bool {
        headers
            .iter()
            .filter(|h| h.field == header.field && h.value == header.value)
            .count() > 0
    }

    #[test]
    fn redirect_with_cookie() {
        let bind = "0.0.0.0:28286";
        let url = format!("http://{}/rcookie", bind);
        let url_bytes = url.clone().into_bytes();
        let h_loc = Header::from_bytes("location", url_bytes.clone()).unwrap();
        let h_setc = Header::from_bytes("set-cookie", "c00k13=data").unwrap();
        let cookie = Header::from_bytes("cookie", "c00k13=data").unwrap();
        let db = Database::open_in_memory().unwrap();

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http(bind).unwrap();
            for r in 0..2 {
                let rq = server.recv().unwrap();
                if rq.url() == "/rcookie" {
                    if r == 0 {
                        let resp = Response::from_string("")
                            .with_status_code(301)
                            .with_header(h_setc.clone())
                            .with_header(h_loc.clone());
                        rq.respond(resp).unwrap();
                    } else if headers_contains(&cookie, rq.headers()) {
                        let resp = Response::from_string("<title>hello<title>")
                            .with_status_code(200);
                        rq.respond(resp).unwrap();
                    }
                }
            }
        });

        thread::sleep(Duration::from_millis(100));
        resolve_url(&url, &Rtd::default(), &db).unwrap();
        server_thread.join().unwrap();
    }
}
