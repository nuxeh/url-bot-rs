use htmlescape::decode_html;
use std::time::Duration;
use itertools::Itertools;
use regex::Regex;
use failure::Error;
use reqwest::Client;
use reqwest::header::{USER_AGENT, ACCEPT_LANGUAGE};
use std::io::Read;
use immeta::{GenericMetadata, load_from_buf};

pub fn resolve_url(url: &str, lang: &str) -> Result<String, Error> {
    eprintln!("RESOLVE {}", url);

    let client = Client::builder()
        .timeout(Duration::from_secs(3)) // per read/write op
        .build()?;

    let resp = client.get(url)
        .header(USER_AGENT, "url-bot-rs/0.1")
        .header(ACCEPT_LANGUAGE, lang)
        .send()?
        .error_for_status()?;

    // Download up to 100KB
    let mut body = Vec::new();
    resp.take(100 * 1024).read_to_end(&mut body)?;

    let contents = String::from_utf8_lossy(&body);

    let title = if let Some(t) = get_image_metadata(&body) {
        Some(t)
    } else if let Some(t) = parse_content(&contents) {
        Some(t)
    } else {
        None
    }.ok_or_else(|| format_err!("failed to parse title"))?;

    Ok(title)
}

fn get_image_metadata(body: &[u8]) -> Option<String> {
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
    return None;
}

fn parse_content(page_contents: &str) -> Option<String> {
    lazy_static! {
        static ref RE: Regex = Regex::new("<title>((.|\n)*?)</title>").unwrap();
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
    fn parse_contents() {
        assert_eq!(None, parse_content(&"".to_string()));
        assert_eq!(None, parse_content(&"    ".to_string()));
        assert_eq!(None, parse_content(&"<title></title>".to_string()));
        assert_eq!(None, parse_content(&"<title>    </title>".to_string()));
        assert_eq!(None,
             parse_content(&"floofynips, not a real webpage".to_string()));
        assert_eq!(Some("cheese is nice".to_string()),
            parse_content(&"<title>cheese is nice</title>".to_string()));
        assert_eq!(Some("squanch".to_string()),
            parse_content(&"<title>     squanch</title>".to_string()));
        assert_eq!(Some("squanch".to_string()),
            parse_content(&"<title>squanch     </title>".to_string()));
        assert_eq!(Some("squanch".to_string()),
            parse_content(&"<title>\nsquanch</title>".to_string()));
        assert_eq!(Some("squanch".to_string()),
            parse_content(&"<title>\n  \n  squanch</title>".to_string()));
        assert_eq!(Some("we like the moon".to_string()),
            parse_content(&"<title>\n  \n  we like the moon</title>".to_string()));
        assert_eq!(Some("&hello123&<>''~".to_string()),
            parse_content(&"<title>&amp;hello123&amp;&lt;&gt;''~</title>".to_string()));
        assert_eq!(Some("CVE - CVE-2018-11235".to_string()),
            parse_content(&"<title>CVE -\nCVE-2018-11235\n</title>".to_string()));
    }
}

