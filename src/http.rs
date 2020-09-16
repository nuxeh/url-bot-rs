use std::{
    time::Duration,
    io::Read,
    thread,
};
use failure::Error;
use reqwest::{
    header::{
        HeaderMap,
        HeaderValue,
        ACCEPT_LANGUAGE,
        ACCEPT_ENCODING,
        CONTENT_TYPE,
    },
    redirect::Policy,
    blocking::{Client, Response}
};
use mime::{Mime, IMAGE, TEXT, HTML};
use humansize::{FileSize, file_size_opts as options};
use log::{debug, trace};
use failure::bail;

use crate::{
    config::Rtd,
    title::{parse_title, get_mime, get_image_metadata}
};

const CHUNK_BYTES: u64 = 100 * 1024; // 100kB
const CHUNKS_MAX: u64 = 10; // 1000kB

pub static DEFAULT_USER_AGENT: &str = concat!(
    "Mozilla/5.0 url-bot-rs",
    "/",
    env!("CARGO_PKG_VERSION"),
);

#[derive(Default)]
pub struct RetrieverBuilder<'a> {
    timeout: Option<Duration>,
    retry_limit: usize,
    retry_delay: Option<Duration>,
    user_agent: Option<&'a str>,
    accept_lang: &'a str,
    redirect_limit: Option<usize>,
}

impl<'a> RetrieverBuilder<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout = Some(Duration::from_secs(timeout_secs));
        self
    }

    pub fn retry(mut self, limit: usize, delay_s: u64) -> Self {
        self.retry_limit = limit;
        self.retry_delay = Some(Duration::from_secs(delay_s));
        self
    }

    pub fn user_agent(mut self, user_agent: &'a str) -> Self {
        self.user_agent = Some(user_agent);
        self
    }

    pub fn accept_lang(mut self, accept_lang: &'a str) -> Self {
        self.accept_lang = accept_lang;
        self
    }

    pub fn redirect_limit(mut self, redirect_limit: usize) -> Self {
        self.redirect_limit = Some(redirect_limit);
        self
    }

    pub fn build(&self) -> Result<Retriever, Error> {
        let mut headers = HeaderMap::new();

        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_str(self.accept_lang)?);
        headers.insert(ACCEPT_ENCODING, HeaderValue::from_static("identity"));

        let user_agent = match self.user_agent {
            Some(u) => u,
            _ => DEFAULT_USER_AGENT,
        };

        let mut builder = Client::builder()
            .cookie_store(true)
            .user_agent(user_agent)
            .default_headers(headers);

        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout)
        };

        if let Some(limit) = self.redirect_limit {
            builder = builder.redirect(Policy::limited(limit))
        };

        let client = builder
            .build()?;

        let retriever = Retriever {
            client,
            retry_limit: self.retry_limit,
            retry_delay: self.retry_delay,
        };

        Ok(retriever)
    }
}

#[derive(Clone)]
pub struct Retriever {
    client: Client,
    retry_limit: usize,
    retry_delay: Option<Duration>,
}

impl Retriever {
    /// Make a request.
    pub fn request(&self, url: &str) -> Result<Response, Error> {
        self.recurse(url, None, 0)
    }

    /// Make a request, providing a HeaderMap of required extra headers to send.
    pub fn request_with_headers(
        &self, url: &str, header_map: HeaderMap
    ) -> Result<Response, Error> {
        self.recurse(url, Some(header_map), 0)
    }

    fn recurse(
        &self,
        url: &str,
        headers: Option<HeaderMap>,
        count: usize
    ) -> Result<Response, Error> {
        let mut client = self.client.get(url);

        if let Some(ref header_map) = headers {
            client = client.headers(header_map.clone())
        };

        let resp = client
            .send()?;

        if count >= self.retry_limit {
            return Ok(resp);
        }

        match (resp.status(), self.retry_delay) {
            (s, _) if s.is_success() => {
                debug!("total requests: {}", count);
                return Ok(resp);
            },

            (s, Some(delay)) if s.is_server_error() => {
                debug!(
                    "server error ({}), retrying in {}s",
                    resp.status(),
                    delay.as_secs()
                );
                thread::sleep(delay);
            },

            _ => {
                let r = resp.error_for_status()?;
                bail!("unhandled request status: {}", r.status());
            },
        };

        // tail recurse any retries
        self.recurse(url, headers, count+1)
    }
}

pub fn resolve_url(url: &str, rtd: &Rtd) -> Result<String, Error> {
    let client = rtd.get_client()?;
    let mut resp = client.request(url)?;
    get_title(&mut resp, rtd, false)
}

pub fn get_title(resp: &mut Response, rtd: &Rtd, dump: bool) -> Result<String, Error> {
    // get content type
    let content_type = resp.headers().get(CONTENT_TYPE)
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
    use crate::{feat, http};
    use std::fs::File;
    use std::path::Path;
    use std::thread;
    use tiny_http::{Response, Header};
    use std::sync::mpsc;

    #[test]
    #[ignore]
    fn resolve_urls() {
        let rtd: Rtd = Rtd::default()
            .init_http_client()
            .unwrap();

        resolve_url("https://youtube.com",  &rtd).unwrap();
        resolve_url("https://google.co.uk", &rtd).unwrap();
    }

    #[test]
    fn resolve_locally_served_files() {
        let files_no_meta = vec![
            "test/img/test.gif",
            "test/other/test.txt",
            "test/other/test.pdf",
        ];

        let mut files_meta = vec![
            ("test/img/test.png", "image/png 800×400"),
            ("test/img/test.jpg", "image/jpeg 400×200"),
            ("test/img/test.gif", "image/gif 1920×1080"),
            ("test/html/basic.html", "basic"),
            ("test/other/test.pdf", "application/pdf 1.31KB"),
        ];

        // not sure why, but served file size is different on windows
        if cfg!(windows) {
            files_meta.push(("test/other/test.txt", "text/plain; charset=utf8 17B"));
        } else {
            files_meta.push(("test/other/test.txt", "text/plain; charset=utf8 16B"));
        };

        let num_files = files_meta.len() + files_no_meta.len();

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http("127.0.0.1:28482").unwrap();
            for _ in 0..num_files {
                let rq = server.recv().unwrap();
                let path = &rq.url()[1..];

                let resp = Response::from_file(File::open(path).unwrap())
                    .with_header(
                        tiny_http::Header {
                            field: "Content-Type".parse().unwrap(),
                            value: get_ctype(&Path::new(path)).parse().unwrap(),
                        }
                    );

                thread::sleep(Duration::from_millis(10));
                rq.respond(resp).unwrap();
            }
        });

        // wait for server thread to be ready
        thread::sleep(Duration::from_millis(1000));

        let mut rtd: Rtd = Rtd::new().init_http_client().unwrap();

        // metadata and mime disabled
        feat!(rtd, report_metadata) = false;
        feat!(rtd, report_mime) = false;

        for t in files_no_meta {
            let url = format!("http://127.0.0.1:28482/{}", t);
            assert!(resolve_url(&url, &rtd).is_err());
        }

        // metadata and mime enabled
        feat!(rtd, report_metadata) = true;
        feat!(rtd, report_mime) = true;

        for t in files_meta {
            let url = format!("http://127.0.0.1:28482/{}", t.0);
            assert_eq!(
                resolve_url(&url, &rtd).unwrap(),
                String::from(t.1)
            )
        }

        server_thread.join().unwrap();
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

        resolve_url(
            "http://127.0.0.1:28282/test",
            &Rtd::new().init_http_client().unwrap()
        ).unwrap();

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

        let res = resolve_url(
            &url,
            &Rtd::new().init_http_client().unwrap()
        );
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

        resolve_url(
            &url,
            &Rtd::new().init_http_client().unwrap()
        ).unwrap();

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

        resolve_url(
            &url,
            &Rtd::new().init_http_client().unwrap()
        ).unwrap();

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

        assert!(
            resolve_url(
                &url,
                &Rtd::new().init_http_client().unwrap()
            ).is_err()
        );

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

        resolve_url(
            &url,
            &Rtd::new().init_http_client().unwrap()
        ).unwrap();

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
        let mut rtd = Rtd::new();
        http!(rtd, max_retries) = 2;
        http!(rtd, retry_delay_s) = 1;
        rtd = rtd.init_http_client().unwrap();

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
