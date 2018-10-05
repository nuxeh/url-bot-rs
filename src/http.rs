extern crate curl;
extern crate htmlescape;

use self::curl::easy::{Easy2, Handler, WriteError, List};
use self::htmlescape::decode_html;

#[derive(Debug)]
struct Collector(Vec<u8>);

impl Handler for Collector {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.0.extend_from_slice(data);
        Ok(data.len())
    }
}

pub fn resolve_url(url: &str, lang: &str) -> Option<String> {

    eprintln!("RESOLVE {}", url);

    let mut easy = Easy2::new(Collector(Vec::new()));

    easy.get(true).unwrap();
    easy.url(url).unwrap();
    easy.follow_location(true).unwrap();
    easy.max_redirections(10).unwrap();
    easy.useragent("url-bot-rs/0.1").unwrap();

    let mut headers = List::new();
    let lang = format!("Accept-Language: {}", lang);
    headers.append(&lang).unwrap();
    easy.http_headers(headers).unwrap();

    match easy.perform() {
        Err(_) => { return None; }
        _      => ()
    }

    let contents = easy.get_ref();

    let s = String::from_utf8_lossy(&contents.0).to_string();

    parse_content(&s)
}

fn parse_content(page_contents: &String) -> Option<String> {

    let s1: Vec<_> = page_contents.split("<title>").collect();
    if s1.len() < 2 { return None }
    let s2: Vec<_> = s1[1].split("</title>").collect();
    if s2.len() < 2 { return None }

    let title_enc = s2[0];

    let title_dec = match decode_html(title_enc) {
        Ok(s) => s,
        _     => {return None}
    };

    /* make any multi-line title string into a single line */
    let mut title_one_line =
        title_dec.lines()
        .fold("".to_string(),
        |string, line| string.to_owned() + " " + line);

    /* trim leading and trailing whitespace */
    title_one_line = title_one_line.trim().to_string();

    match title_one_line.is_empty() {
        false => {eprintln!("SUCCESS \"{}\"", title_one_line);
                 Some(title_one_line)},
        true  => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_urls() {
        assert_ne!(None, resolve_url("https://youtube.com", "en"));
        assert_ne!(None, resolve_url("https://google.co.uk", "en"));
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

