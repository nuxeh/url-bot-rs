use std::time::Duration;
use failure::Error;
use reqwest::{
    header,
    redirect::Policy,
    blocking::{Client, Response}
};
use std::io::Read;
use std::thread;
use mime::{Mime, IMAGE, TEXT, HTML};
use humansize::{FileSize, file_size_opts as options};
use log::{debug, trace};
use failure::bail;

use super::http;
use super::config::Rtd;
use super::title::{parse_title, get_mime, get_image_metadata};

const CHUNK_BYTES: u64 = 100 * 1024; // 100kB
const CHUNKS_MAX: u64 = 10; // 1000kB

static DEFAULT_USER_AGENT: &str = concat!(
    "Mozilla/5.0 url-bot-rs",
    "/",
    env!("CARGO_PKG_VERSION"),
);

pub struct Session<'a> {
    client: Client,
    rtd: &'a Rtd,
    url: Option<&'a str>,
    request_count: u8,
}

impl<'a> Session<'a> {
    pub fn new(rtd: &'a Rtd) -> Session<'a> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::ACCEPT_LANGUAGE,
            header::HeaderValue::from_str(&http!(rtd, accept_lang)).unwrap()
        );
        headers.insert(header::ACCEPT_ENCODING, header::HeaderValue::from_static("identity"));

        let user_agent = match &http!(rtd, user_agent) {
            Some(u) => u,
            _ => DEFAULT_USER_AGENT,
        };

        let client = Client::builder()
            .cookie_store(true)
            .default_headers(headers)
            .redirect(Policy::limited(http!(rtd, max_redirections).into()))
            .timeout(Duration::from_secs(http!(rtd, timeout_s)))
            .user_agent(user_agent)
            .build()
            .expect("Can't build reqwest client");

        Session {
            client,
            rtd,
            url: None,
            request_count: 0,
        }
    }

    pub fn url(&mut self, url: &'a str) -> &mut Self {
        self.url = Some(url);
        self
    }

    pub fn request(&mut self) -> Result<Response, Error> {
        self.request_count += 1;

        let resp = match self.url {
            Some(url) => {
                self.client.get(url).send()?
            }
            None => bail!("Missing URL"),
        };

        if self.request_count > http!(self.rtd, max_retries) {
            return Ok(resp);
        }

        let status = resp.status();

        if status.is_success() {
            debug!("total requests: {}", self.request_count);
            return Ok(resp);
        } else if status.is_server_error() {
            let delay = http!(self.rtd, retry_delay_s);
            debug!("server error ({}), retrying in {}s", resp.status(), delay);
            thread::sleep(Duration::from_secs(delay));
        } else {
            let r = resp.error_for_status()?;
            bail!("unhandled request status: {}", r.status());
        };

        // tail recurse any retries
        self.request()
    }
}

pub fn resolve_url(url: &str, rtd: &Rtd) -> Result<String, Error> {
    let mut resp = Session::new(rtd).url(url).request()?;
    get_title(&mut resp, rtd, false)
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

/// HTTP tests
///
/// In these tests, local HTTP servers are run on threads to serve content to
/// "main" test threads, in known ways seen in the wild, e.g. expecting
/// redirection, and redirection after setting a cookie.
///
/// In some cases, made only more hairy by the fact that Rust tests run
/// concurrently on their own threads anyway, the result is a lot of threads
/// sending and receiving data to eachother, in close proximity, on localhost,
/// simultaneuously, and accepting no failures. At the very least this has the
/// potential to hammer IO in tight request loops, and enough to cause
/// failures.
///
/// For this reason, the tests are peppered with delays to throttle responses
/// from requests slightly (10ms is a fast response from an internet server in
/// any case), throtting between test runs, and delays to allow server threads
/// to spawn. It looks hacky, but seems to work for now.
///
/// In the future a better way to choose an inconspicuous, unused port for
/// each test could be advantageous, too.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feat;
    use std::fs::File;
    use std::path::{Path, PathBuf};
    use std::thread;
    use tiny_http::{Response, Header};
    use std::sync::mpsc;

    #[test]
    #[ignore]
    fn resolve_urls() {
        let rtd: Rtd = Rtd::default();
        resolve_url("https://youtube.com",  &rtd).unwrap();
        resolve_url("https://google.co.uk", &rtd).unwrap();
    }

    #[test]
    fn resolve_locally_served_files() {
        let mut rtd: Rtd = Rtd::default();

        // metadata and mime disabled
        feat!(rtd, report_metadata) = false;
        feat!(rtd, report_mime) = false;

        let files = vec![
            "test/img/test.gif",
            "test/other/test.txt",
            "test/other/test.pdf",
        ];

        for t in files {
            assert!(serve_resolve(PathBuf::from(t), &rtd).is_err());
        }

        // metadata and mime enabled
        feat!(rtd, report_metadata) = true;
        feat!(rtd, report_mime) = true;

        let mut files = vec![
            ("test/img/test.png", "image/png 800×400"),
            ("test/img/test.jpg", "image/jpeg 400×200"),
            ("test/img/test.gif", "image/gif 1920×1080"),
            ("test/html/basic.html", "basic"),
            ("test/other/test.pdf", "application/pdf 1.31KB"),
        ];

        // not sure why, but served file size is different on windows
        if cfg!(windows) {
            files.push(("test/other/test.txt", "text/plain; charset=utf8 17B"));
        } else {
            files.push(("test/other/test.txt", "text/plain; charset=utf8 16B"));
        };

        for t in files {
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
            let server = tiny_http::Server::http("127.0.0.1:28482").unwrap();
            let rq = server.recv().unwrap();
            if rq.url() == "/test" {
                let resp = Response::from_file(File::open(&path).unwrap())
                    .with_header(
                        tiny_http::Header {
                            field: "Content-Type".parse().unwrap(),
                            value: get_ctype(&path).parse().unwrap(),
                        }
                    );
                thread::sleep(Duration::from_millis(10));
                rq.respond(resp).unwrap();
            }
        });

        // wait for server thread to be ready
        thread::sleep(Duration::from_millis(50));

        let res = resolve_url("http://127.0.0.1:28482/test", &rtd);
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
        let expected: Vec<Header> = vec![
            Header::from_bytes("cookie", "").unwrap(),
            Header::from_bytes("user-agent", DEFAULT_USER_AGENT).unwrap(),
            Header::from_bytes("accept-language", "en").unwrap(),
            Header::from_bytes("accept-encoding", "identity").unwrap(),
            Header::from_bytes("accept", "*/*").unwrap(),
            Header::from_bytes("host", "127.0.0.1:28282").unwrap(),
        ];

        let (tx, rx) = mpsc::channel();
        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http("127.0.0.1:28282").unwrap();
            let rq = server.recv().unwrap();
            if rq.url() == "/test" {
                // send headers through mpsc channel
                tx.send(rq.headers().to_owned()).unwrap();

                // respond with some content
                let path = Path::new("./test/html/basic.html");
                let resp = Response::from_file(File::open(path).unwrap());
                thread::sleep(Duration::from_millis(10));
                rq.respond(resp).unwrap();
            }
        });

        // wait for server thread to be ready
        thread::sleep(Duration::from_millis(50));

        resolve_url("http://127.0.0.1:28282/test", &Rtd::default()).unwrap();
        let request_headers = rx.recv().unwrap();

        println!("Headers in request:\n{:#?}", request_headers);
        println!("Headers expected:\n{:#?}", expected);

        let headers_match = request_headers
            .iter()
            .all(|header| {
                expected
                    .iter()
                    .fold(false, |acc, v| {
                        acc || v.field == header.field && v.value == header.value
                    })
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
        let bind = "127.0.0.1:28270";
        let url = format!("http://{}/rlim", bind);
        let url_bytes = url.clone().into_bytes();
        let header = Header::from_bytes("location", url_bytes.clone()).unwrap();
        let timeout = Duration::from_millis(200);

        // throttle between runs
        thread::sleep(Duration::from_millis(50));

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http(bind).unwrap();

            // send redirections
            for _ in 1..n {
                let rq = server.recv().unwrap();
                if rq.url() == "/rlim" {
                    let resp = Response::from_string("")
                        .with_status_code(status)
                        .with_header(header.clone());
                    thread::sleep(Duration::from_millis(10));
                    rq.respond(resp).unwrap();
                }
            }

            // send success. if the resolve function errors, it will send no
            // more requests, so time out
            if let Ok(Some(rq)) = server.recv_timeout(timeout) {
                let resp = Response::from_string("<title>hello<title>")
                    .with_status_code(200);
                thread::sleep(Duration::from_millis(10));
                rq.respond(resp).unwrap();
            }
        });

        // wait for server thread to be ready
        thread::sleep(Duration::from_millis(50));

        let res = resolve_url(&url, &Rtd::default());
        server_thread.join().unwrap();
        res
    }

    #[test]
    fn redirect_absolute_location() {
        let bind = "127.0.0.1:28280";
        let url = format!("http://{}/r_abs", bind);
        let url2 = format!("http://{}/r_abs_r", bind);
        let url2_bytes = url2.clone().into_bytes();
        let h_loc = Header::from_bytes("location", url2_bytes.clone()).unwrap();

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
                        thread::sleep(Duration::from_millis(10));
                        rq.respond(resp).unwrap();
                    },
                    // response
                    "/r_abs_r" => {
                        let resp = Response::from_string("<title>hello</title>");
                        thread::sleep(Duration::from_millis(10));
                        rq.respond(resp).unwrap();
                    },
                    _ => (),
                }
            }
        });

        // wait for server thread to be ready
        thread::sleep(Duration::from_millis(50));

        resolve_url(&url, &Rtd::default()).unwrap();
        server_thread.join().unwrap();
    }

    #[test]
    fn redirect_relative_location() {
        let bind = "127.0.0.1:28278";
        let url = format!("http://{}/r_rel", bind);
        let h_loc = Header::from_bytes("location", "/r_rel_r").unwrap();

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
                        thread::sleep(Duration::from_millis(10));
                        rq.respond(resp).unwrap();
                    },
                    // response
                    "/r_rel_r" => {
                        let resp = Response::from_string("<title>hello</title>");
                        thread::sleep(Duration::from_millis(10));
                        rq.respond(resp).unwrap();
                    },
                    _ => (),
                }
            }
        });

        // wait for server thread to be ready
        thread::sleep(Duration::from_millis(50));

        resolve_url(&url, &Rtd::default()).unwrap();
        server_thread.join().unwrap();
    }

    #[test]
    fn redirect_no_redirection_location_provided() {
        let bind = "127.0.0.1:28288";
        let url = format!("http://{}/rerr", bind);

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http(bind).unwrap();
            let rq = server.recv().unwrap();
            if rq.url() == "/rerr" {
                let resp = Response::from_string("<title>hello</title>")
                    .with_status_code(301);
                thread::sleep(Duration::from_millis(10));
                rq.respond(resp).unwrap();
            }
        });

        // wait for server thread to be ready
        thread::sleep(Duration::from_millis(50));

        assert!(resolve_url(&url, &Rtd::default()).is_err());
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
        let bind = "127.0.0.1:28286";
        let url = format!("http://{}/rcookie", bind);
        let url_bytes = url.clone().into_bytes();
        let h_loc = Header::from_bytes("location", url_bytes.clone()).unwrap();
        let h_setc = Header::from_bytes("set-cookie", "c00k13=data").unwrap();
        let cookie = Header::from_bytes("cookie", "c00k13=data").unwrap();

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
                        thread::sleep(Duration::from_millis(10));
                        rq.respond(resp).unwrap();
                    } else if headers_contains(&cookie, rq.headers()) {
                        let resp = Response::from_string("<title>hello<title>")
                            .with_status_code(200);
                        thread::sleep(Duration::from_millis(10));
                        rq.respond(resp).unwrap();
                    }
                }
            }
        });

        // wait for server thread to be ready
        thread::sleep(Duration::from_millis(50));

        resolve_url(&url, &Rtd::default()).unwrap();
        server_thread.join().unwrap();
    }

    #[test]
    fn test_retry_server_errors() {
        // 500 Internal Server Error https://http.cat/500
        test_retry_server_errors_n_status(1, 500).unwrap();
        test_retry_server_errors_n_status(2, 500).unwrap();
        assert!(test_retry_server_errors_n_status(3, 500).is_err());

        // 503 Service Unavailable https://http.cat/503
        test_retry_server_errors_n_status(1, 503).unwrap();
        test_retry_server_errors_n_status(2, 503).unwrap();
        assert!(test_retry_server_errors_n_status(3, 503).is_err());

        // 504 Gateway Timeout https://http.cat/504
        test_retry_server_errors_n_status(1, 504).unwrap();
        test_retry_server_errors_n_status(2, 504).unwrap();
        assert!(test_retry_server_errors_n_status(3, 504).is_err());
    }

    fn test_retry_server_errors_n_status(n: usize, status: u16) -> Result<String, Error> {
        let bind = "127.0.0.1:28268";
        let url = format!("http://{}/serr", bind);
        let timeout = Duration::from_secs(2);
        let mut rtd = Rtd::default();
        http!(rtd, max_retries) = 2;
        http!(rtd, retry_delay_s) = 1;

        // throttle between runs
        thread::sleep(Duration::from_millis(50));

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http(bind).unwrap();

            // return server errors
            // TODO: test that the client uses a delay before repeat requests
            for _ in 0..n {
                let rq = server.recv().unwrap();
                let resp = Response::from_string("")
                    .with_status_code(status);
                thread::sleep(Duration::from_millis(10));
                rq.respond(resp).unwrap();
            }

            // send success. if the resolve function errors, it will send no
            // more requests, so time out
            if let Ok(Some(rq)) = server.recv_timeout(timeout) {
                let resp = Response::from_string("<title>hello<title>")
                    .with_status_code(200);
                thread::sleep(Duration::from_millis(10));
                rq.respond(resp).unwrap();
            }
        });

        // wait for server thread to be ready
        thread::sleep(Duration::from_millis(50));

        let res = resolve_url(&url, &rtd);
        server_thread.join().unwrap();
        res
    }
}
