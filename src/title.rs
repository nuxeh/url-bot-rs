use itertools::Itertools;
use image::ImageFormat;
use image::io::Reader;
use mime::Mime;
use scraper::{Html, Selector};
use std::io::Cursor;

use super::config::Rtd;

/// Format a mime string
pub fn get_mime(rtd: &Rtd, mime: &Mime, size: &str) -> Option<String> {
    if rtd.conf.features.report_mime {
        Some(format!("{} {}", mime, size.replace(" ", "")))
    } else {
        None
    }
}

fn get_image_mime(format: ImageFormat) -> Option<Mime> {
    let mime_str = match format {
        ImageFormat::PNG => Some("image/png"),
        ImageFormat::JPEG => Some("image/jpeg"),
        ImageFormat::GIF => Some("image/gif"),
        ImageFormat::PNM => Some("image/x-portable-anymap"),
        ImageFormat::TIFF => Some("image/tiff"),
        ImageFormat::BMP => Some("image/bmp"),
        _ => None,
    };

    mime_str.map(|s| s.parse().expect("invalid mime"))
}

/// Attempt to get metadata from an image
pub fn get_image_metadata(rtd: &Rtd, body: &[u8]) -> Option<String> {
    if !rtd.conf.features.report_metadata {
        return None;
    }

    let reader = Reader::new(Cursor::new(body))
        .with_guessed_format()
        .expect("failed to create image::Reader");

    let mime = reader.format().and_then(get_image_mime);

    match (mime, reader.into_dimensions()) {
        (Some(m), Ok((w, h))) => Some(format!("{} {}×{}", m.to_string(), w, h)),
        _ => None,
    }
}

/// Attempt to parse HTML for a page title
fn parse_html_title(page_contents: &str) -> Option<String> {
    let fragment = Html::parse_document(page_contents);
    let title_selector = Selector::parse("title").unwrap();

    fragment
        .select(&title_selector)
        .next()
        .map(|n| n.text().collect())
}

/// Attempt to extract a page title from downloaded HTML
pub fn parse_title(page_contents: &str) -> Option<String> {
    let title_dec = match parse_html_title(page_contents) {
        Some(t) => t,
        None => return None,
    };

    // make any multi-line title string into a single line,
    // trim leading and trailing whitespace
    let title_one_line = title_dec
        .trim()
        .lines()
        .map(str::trim)
        .join(" ");

    if title_one_line.is_empty() {
        return None;
    }

    Some(title_one_line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::path::Path;
    use std::io::Read;

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
        assert_eq!(
            Some(String::from("this title contains &")),
            parse_title("<title>this title contains &</title>")
        );
    }

    #[test]
    fn get_metadata_from_local_images() {
        for test in vec!(
            ("./test/img/test.png", "image/png 800×400"),
            ("./test/img/test.jpg", "image/jpeg 400×200"),
            ("./test/img/test.gif", "image/gif 1920×1080"),
            ("./test/img/test.bmp", "image/bmp 19×19"),
            ("./test/img/test.pnm", "image/x-portable-anymap 22×22"),
            ("./test/img/test.pgm", "image/x-portable-anymap 24×24"),
            ("./test/img/test.tiff", "image/tiff 4×4")
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
}
