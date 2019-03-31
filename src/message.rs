use irc::client::prelude::*;
use std::iter;
use unicode_segmentation::UnicodeSegmentation;
use reqwest::Url;
use regex::Regex;

use super::http::resolve_url;
use super::sqlite::{Database, NewLogEntry};
use super::config::Rtd;
use super::tld::TLD;

pub fn handle_message(
    client: &IrcClient, message: &Message, rtd: &Rtd, db: &Database
) {
    trace!("{:?}", message.command);

    // match on message type
    let (target, msg) = match message.command {
        Command::PRIVMSG(ref target, ref msg) => (target, msg),
        _ => return,
    };

    let is_chanmsg = target.starts_with('#');
    let user = message.source_nickname().unwrap();
    let mut num_processed = 0;

    // look at each space-separated message token
    for token in msg.split_whitespace() {
        // the token must not contain unsafe characters
        if contains_unsafe_chars(token) {
            continue;
        }

        // get a full URL for tokens without a scheme
        let maybe_token = add_scheme_for_tld(token);
        let token = maybe_token
            .as_ref()
            .map_or(token, String::as_str);

        // the token must be a valid url
        let url = match token.parse::<Url>() {
            Ok(url) => url,
            _ => continue,
        };

        // the scheme must be http or https
        if !["http", "https"].contains(&url.scheme()) {
            continue;
        }

        info!("RESOLVE <{}>", token);

        // try to get the title from the url
        let title = match resolve_url(token, rtd, db) {
            Ok(title) => title,
            Err(err) => {
                error!("{:?}", err);
                continue
            },
        };

        // create a log entry struct
        let entry = NewLogEntry {
            title: &title,
            url: token,
            user,
            channel: target,
        };

        // check for pre-post
        let pre_post = if rtd.history {
            db.check_prepost(token)
        } else {
            Ok(None)
        };

        // generate response string
        let mut msg = match pre_post {
            Ok(Some(previous_post)) => {
                let user = if rtd.conf.features.mask_highlights {
                    create_non_highlighting_name(&previous_post.user)
                } else {
                    previous_post.user
                };
                format!("⤷ {} → {} {} ({})",
                    title,
                    previous_post.time_created,
                    user,
                    previous_post.channel
                )
            },
            Ok(None) => {
                // add new log entry to database
                if rtd.history && is_chanmsg {
                    if let Err(err) = db.add_log(&entry) {
                        error!("SQL error: {}", err);
                    }
                }
                format!("⤷ {}", title)
            },
            Err(err) => {
                error!("SQL error: {}", err);
                continue
            },
        };

        // limit response length, see RFC1459
        msg = utf8_truncate(&msg, 510);

        // log
        info!("{}", msg);

        // send the IRC response
        let target = message.response_target().unwrap_or(target);
        if rtd.conf.features.send_notice && is_chanmsg {
            client.send_notice(target, &msg).unwrap()
        } else {
            client.send_privmsg(target, &msg).unwrap()
        }

        // limit the number of processed URLs
        num_processed += 1;
        if num_processed == rtd.conf.params.url_limit {
            break;
        }
    };
}

// regex for unsafe characters, as defined in RFC 1738
const RE_UNSAFE_CHARS: &str = r#"[{}|\\^~\[\]`<>"]"#;

/// does the token contain characters not permitted by RFC 1738
fn contains_unsafe_chars(token: &str) -> bool {
    lazy_static! {
        static ref UNSAFE: Regex = Regex::new(RE_UNSAFE_CHARS).unwrap();
    }
    UNSAFE.is_match(token)
}

/// create a name that doesn't trigger highlight regexes
fn create_non_highlighting_name(name: &str) -> String {
    let mut graphemes = name.graphemes(true);
    let first = graphemes.next();

    first
        .into_iter()
        .chain(iter::once("\u{200C}"))
        .chain(graphemes)
        .collect()
}

/// truncate to a maximum number of bytes, taking UTF-8 into account
fn utf8_truncate(s: &str, n: usize) -> String {
    s.char_indices()
        .take_while(|(len, c)| len + c.len_utf8() <= n)
        .map(|(_, c)| c)
        .collect()
}

/// if a token has a recognised TLD, but no scheme, add one
fn add_scheme_for_tld(token: &str) -> Option<String> {
    if token.parse::<Url>().is_err() {
        let new_token = format!("http://{}", token);

        if let Ok(url) = new_token.parse::<Url>() {
            if !url.domain()?.contains('.') {
                return None;
            }

            let tld = url.domain()?
                .split('.')
                .last()?;

            if TLD.contains(tld) {
                return Some(new_token);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_truncate() {
        assert_eq!("", utf8_truncate("", 10));
        assert_eq!("", utf8_truncate("", 1));
        assert_eq!(" ", utf8_truncate("  ", 1));
        assert_eq!("\u{2665}", utf8_truncate("\u{2665}", 4));
        assert_eq!("\u{2665}", utf8_truncate("\u{2665}", 3));
        assert_eq!("", utf8_truncate("\u{2665}", 2));
        assert_eq!("\u{0306}\u{0306}", utf8_truncate("\u{0306}\u{0306}", 4));
        assert_eq!("\u{0306}", utf8_truncate("\u{0306}\u{0306}", 2));
        assert_eq!("\u{0306}", utf8_truncate("\u{0306}", 2));
        assert_eq!("", utf8_truncate("\u{0306}", 1));
        assert_eq!("hello ", utf8_truncate("hello \u{1F603} world!", 9));
    }

    #[test]
    fn test_create_non_highlighting_name() {
        assert_eq!("\u{200C}", create_non_highlighting_name(""));
        assert_eq!("f\u{200C}oo", create_non_highlighting_name("foo"));
        assert_eq!("b\u{200C}ar", create_non_highlighting_name("bar"));
        assert_eq!("b\u{200C}az", create_non_highlighting_name("baz"));
    }

    #[test]
    fn test_contains_unsafe_chars() {
        for c in &['{', '}', '|', '\\', '^', '~', '[', ']', '`', '<', '>', '"']
        {
            assert!(contains_unsafe_chars(&format!("http://z/{}", c)));
        }
        assert_eq!(contains_unsafe_chars("http://z.zzz/"), false);
    }

    #[test]
    fn test_add_scheme_for_tld() {
        // appears to be a URL, and has a valid TLD
        assert!(add_scheme_for_tld("docs.rs").is_some());
        assert!(add_scheme_for_tld("nomnomnom.xyz").is_some());
        assert!(add_scheme_for_tld("endless.horse").is_some());
        assert!(add_scheme_for_tld("google.co.uk").is_some());
        assert!(add_scheme_for_tld("notreal.co.uk/#banana").is_some());
        assert!(add_scheme_for_tld("notreal.co.uk/?banana=3").is_some());

        // return value is as expected
        assert_eq!(
            Some(String::from("http://nomnomnom.xyz")),
            add_scheme_for_tld("nomnomnom.xyz")
        );
        assert_eq!(
            Some(String::from("http://google.co.uk")),
            add_scheme_for_tld("google.co.uk")
        );

        // already a valid URL
        assert!(add_scheme_for_tld("http://nomnomnom.xyz").is_none());
        assert!(add_scheme_for_tld("http://endless.horse").is_none());

        // not a recognised TLD
        assert!(add_scheme_for_tld("abc.cheese").is_none());
        assert!(add_scheme_for_tld("abc.limes").is_none());

        // recognised TLD, but incomplete as a URL
        assert!(add_scheme_for_tld("xyz").is_none());
        assert!(add_scheme_for_tld("uk").is_none());
        assert!(add_scheme_for_tld("horse").is_none());
    }
}
