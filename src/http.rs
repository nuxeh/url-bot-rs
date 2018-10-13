use htmlescape::decode_html;
use std::time::Duration;
use itertools::Itertools;
use regex::Regex;
use failure::Error;
use reqwest::Client;
use reqwest::header::{USER_AGENT, ACCEPT_LANGUAGE, CONTENT_TYPE, CONTENT_LENGTH};
use std::io::Read;
use immeta::{GenericMetadata, load_from_buf};
use mime::{Mime, IMAGE, TEXT, HTML};
use humansize::{FileSize, file_size_opts as options};
use config::ConfOpts;

const DOWNLOAD_SIZE: u64 = 100 * 1024; // 100kB

pub fn resolve_url(url: &str, lang: &str, conf: &ConfOpts) -> Result<String, Error> {
    eprintln!("RESOLVE {}", url);

    let client = Client::builder()
        .timeout(Duration::from_secs(10)) // per read/write op
        .build()?;

    let resp = client.get(url)
        .header(USER_AGENT, "url-bot-rs/0.1")
        .header(ACCEPT_LANGUAGE, lang)
        .send()?
        .error_for_status()?;

    // Get some response headers
    let content_type = resp.headers().get(CONTENT_TYPE)
        .and_then(|typ| typ.to_str().ok())
        .and_then(|typ| typ.parse::<Mime>().ok());
    let len = resp.headers().get(CONTENT_LENGTH)
        .and_then(|len| len.to_str().ok())
        .and_then(|len| len.parse().ok())
        .unwrap_or(0);
    let size = len.file_size(options::CONVENTIONAL).unwrap_or(String::new());

    // Download body
    let mut body = Vec::new();
    let bytes = match content_type.clone() {
        Some(ct) => {
            match (ct.type_(), ct.subtype()) {
                (IMAGE, _) => 10 * 1024 * 1024, // 10MB
                _ => DOWNLOAD_SIZE,
            }
        },
        None => DOWNLOAD_SIZE,
    };
    resp.take(bytes).read_to_end(&mut body)?;
    let contents = String::from_utf8_lossy(&body);

    // Get title or metadata
    let title = match content_type {
        Some(ct) => {
            match (ct.type_(), ct.subtype()) {
                (IMAGE, _) => parse_title(&contents)
                    .or(get_image_metadata(&conf, &body))
                    .or(get_mime(&conf, &ct, &size)),
                (TEXT, HTML) => parse_title(&contents),
                _ => parse_title(&contents)
                    .or(get_mime(&conf, &ct, &size)),
            }
        },
        None => parse_title(&contents),
    }.ok_or_else(|| format_err!("failed to parse title"))?;

    Ok(title)
}

fn get_mime(conf: &ConfOpts, c_type: &Mime, size: &str) -> Option<String> {
    match conf.report_mime {
        Some(true) => Some(format!("{} {}", c_type, size.replace(" ", ""))),
        _ => None
    }
}

fn get_image_metadata(conf: &ConfOpts, body: &[u8]) -> Option<String> {
    if !conf.report_metadata.unwrap() {
        return None;
    };
    if let Ok(img_meta) = load_from_buf(&body) {
        return match img_meta {
            GenericMetadata::Jpeg(m) => Some(format!("image/jpeg {}×{}",
                m.dimensions.width, m.dimensions.height)),
            GenericMetadata::Gif(m) => Some(format!("image/gif {}×{}",
                m.dimensions.width, m.dimensions.height)),
            GenericMetadata::Png(m) => Some(format!("image/png {}×{}",
                m.dimensions.width, m.dimensions.height)),
            _ => None,
        };
    };
    None
}

fn parse_title(page_contents: &str) -> Option<String> {
    lazy_static! {
        static ref RE: Regex = Regex::new("<title.*>((.|\n)*?)</title>").unwrap();
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

    eprintln!("SUCCESS \"{}\"", title_one_line);
    Some(title_one_line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_urls() {
        resolve_url("https://youtube.com", "en").unwrap();
        resolve_url("https://google.co.uk", "en").unwrap();
    }

    #[test]
    fn parse_titles() {
        assert_eq!(None, parse_title(&"".to_string()));
        assert_eq!(None, parse_title(&"    ".to_string()));
        assert_eq!(None, parse_title(&"<title></title>".to_string()));
        assert_eq!(None, parse_title(&"<title>    </title>".to_string()));
        assert_eq!(None,
             parse_title(&"floofynips, not a real webpage".to_string()));
        assert_eq!(Some("cheese is nice".to_string()),
            parse_title(&"<title>cheese is nice</title>".to_string()));
        assert_eq!(Some("squanch".to_string()),
            parse_title(&"<title>     squanch</title>".to_string()));
        assert_eq!(Some("squanch".to_string()),
            parse_title(&"<title>squanch     </title>".to_string()));
        assert_eq!(Some("squanch".to_string()),
            parse_title(&"<title>\nsquanch</title>".to_string()));
        assert_eq!(Some("squanch".to_string()),
            parse_title(&"<title>\n  \n  squanch</title>".to_string()));
        assert_eq!(Some("we like the moon".to_string()),
            parse_title(&"<title>\n  \n  we like the moon</title>".to_string()));
        assert_eq!(Some("&hello123&<>''~".to_string()),
            parse_title(&"<title>&amp;hello123&amp;&lt;&gt;''~</title>".to_string()));
        assert_eq!(Some("CVE - CVE-2018-11235".to_string()),
            parse_title(&"<title>CVE -\nCVE-2018-11235\n</title>".to_string()));
    }
}

