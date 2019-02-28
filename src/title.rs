use scraper::{Html, Selector};

/// Attempt to parse HTML for a page title
fn parse_html_title(page_contents: &str) -> Option<String> {
    let fragment = Html::parse_document(page_contents);
    let title_selector = Selector::parse("title").unwrap();

    fragment
        .select(&title_selector)
        .next()
        .and_then(|n| Some(n.text().collect()))
}
