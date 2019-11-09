use irc::client::prelude::*;
use std::iter;
use std::collections::HashSet;
use unicode_segmentation::UnicodeSegmentation;
use reqwest::Url;
use regex::Regex;

use super::http::resolve_url;
use super::sqlite::{Database, NewLogEntry};
use super::config::Rtd;
use super::tld::TLD;

pub fn handle_message(client: &IrcClient, message: &Message, rtd: &mut Rtd, db: &Database) {
    trace!("{:?}", message.command);

    match message.command {
        Command::KICK(ref chan, ref nick, _) => kick(client, rtd, chan, nick),
        Command::INVITE(ref nick, ref chan) => invite(client, rtd, nick, chan),
        Command::PRIVMSG(ref target, ref msg) => {
            let sender = message.source_nickname().unwrap();
            let target = message.response_target().unwrap_or(target);
            let message = Msg::new(rtd, sender, target, msg);
            privmsg(client, rtd, db, &message)
        },
        _ => {},
    };
}

fn kick(client: &IrcClient, rtd: &mut Rtd, chan: &str, nick: &str) {
    if !rtd.conf.features.autosave {
        return;
    }

    if nick != client.current_nickname() {
        return;
    }

    info!("kicked from {}", chan);

    rtd.conf.remove_channel(chan);
    rtd.conf.write(&rtd.paths.conf).unwrap_or_else(|err| {
        error!("error writing config: {}", err);
    });
}

fn invite(client: &IrcClient, rtd: &mut Rtd, nick: &str, chan: &str) {
    if !rtd.conf.features.invite {
        return;
    }

    if nick != client.current_nickname() {
        return;
    }

    info!("invited to channel: {}", chan);

    if let Err(e) = client.send_join(chan) {
        error!("error joining channel: {}", e);
    } else {
        info!("joined {}", chan);

        if rtd.conf.features.autosave {
            rtd.conf.add_channel(chan.to_string());
            rtd.conf.write(&rtd.paths.conf).unwrap_or_else(|err| {
                error!("error writing config: {}", err);
            });
        };
    };
}

enum TitleResp {
    TITLE(String),
    ERROR(String),
}

struct Msg {
    is_chanmsg: bool,
    is_ping: bool,
    target: String,
    sender: String,
    text: String,
}

impl Msg {
    fn new(rtd: &Rtd, sender: &str, target: &str, text: &str) -> Msg {
        let our_nick = rtd.conf.client.nickname.as_ref().unwrap();

        Msg {
            is_chanmsg: target.starts_with('#'),
            is_ping: is_ping(&our_nick, text),
            sender: sender.to_string(),
            target: target.to_string(),
            text: text.to_string(),
        }
    }
}

fn privmsg(client: &IrcClient, rtd: &Rtd, db: &Database, msg: &Msg) {
    let titles: Vec<_> = process_titles(rtd, db, msg).collect();

    for resp in &titles {
        match resp {
            TitleResp::TITLE(t) => {
                respond(client, rtd, msg, t);
            },
            TitleResp::ERROR(ref e) => {
                respond_error(client, rtd, msg, e);
            },
        }
    }

    // if we had no url message and got a ping send nick response
    if titles.is_empty() && msg.is_ping {
        respond(client, rtd, &msg, &rtd.conf.params.nick_response_str);
    }

}

/// find titles in a message and generate responses
fn process_titles(rtd: &Rtd, db: &Database, msg: &Msg) -> impl Iterator<Item = TitleResp> {
    let mut responses: Vec<TitleResp> = vec![];

    let mut num_processed = 0;
    let mut dedup_urls = HashSet::new();

    // look at each space-separated message token
    for token in msg.text.split_whitespace() {
        // the token must not contain unsafe characters
        if contains_unsafe_chars(token) {
            continue;
        }

        // get a full URL for tokens without a scheme
        let maybe_token = if rtd.conf.features.partial_urls {
            add_scheme_for_tld(token)
        } else {
            None
        };

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

        // skip duplicate urls within the message
        if dedup_urls.contains(&url) {
            continue;
        }

        info!("RESOLVE <{}>", token);

        // try to get the title from the url
        let title = match resolve_url(token, rtd, db) {
            Ok(title) => title,
            Err(err) => {
                error!("{:?}", err);
                responses.push(TitleResp::ERROR(err.to_string()));
                continue;
            },
        };

        // create a log entry struct
        let entry = NewLogEntry {
            title: &title,
            url: token,
            user: &msg.sender,
            channel: &msg.target,
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
                // add new log entry to database, if posted in a channel
                if rtd.history && msg.is_chanmsg {
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

        responses.push(TitleResp::TITLE(msg.to_string()));

        dedup_urls.insert(url);

        // limit the number of processed URLs
        num_processed += 1;
        if num_processed == rtd.conf.params.url_limit {
            break;
        }
    };

    responses.into_iter()
}

/// send IRC response
fn respond<S>(client: &IrcClient, rtd: &Rtd, msg: &Msg, text: S)
where
    S: ToString + std::fmt::Display,
{
    let result = if rtd.conf.features.send_notice && msg.is_chanmsg {
        client.send_notice(&msg.target, &text)
    } else {
        client.send_privmsg(&msg.target, &text)
    };

    result.unwrap_or_else(|err| {
        error!("Error sending response {}: {}", msg.target, err);
    });
}

fn respond_error<S>(client: &IrcClient, rtd: &Rtd, msg: &Msg, text: S)
where
    S: ToString + std::fmt::Display,
{
    if rtd.conf.features.reply_with_errors {
        respond(client, rtd, &msg, &text);
    };

    if rtd.conf.features.send_errors_to_poster {
        respond(client, rtd, &msg, &text);
    };

    msg_status_chans(client, rtd, &text);
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

/// does a message look like it contains a ping
fn is_ping(nick: &str, message: &str) -> bool {
    let regex = format!(r#"\b{}\b"#, nick);
    let ping = Regex::new(&regex).unwrap();
    ping.is_match(message)
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
pub fn add_scheme_for_tld(token: &str) -> Option<String> {
    if token.parse::<Url>().is_err() {
        if token.starts_with('@') {
            return None;
        }

        let new_token = format!("http://{}", token);

        if let Ok(url) = new_token.parse::<Url>() {
            if !url.domain()?.contains('.') {
                return None;
            }

            // reject email addresses
            if url.username() != "" {
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

/// join any status channels not already joined and send a message to them
pub fn msg_status_chans<S>(client: &IrcClient, rtd: &Rtd, msg: S)
where
    S: ToString + std::fmt::Display,
{
    if rtd.conf.params.status_channels.is_empty() {
        return;
    };

    let joined_channels = client.list_channels().unwrap_or_else(|| vec![]);

    rtd.conf.params.status_channels
        .iter()
        .filter(|c| !joined_channels.contains(c))
        .for_each(|c| client.send_join(c).unwrap_or_else(|err| {
            error!("Error joining status channel {}: {}", c, err)
        }));

    rtd.conf.params.status_channels
        .iter()
        .for_each(|c| client.send_privmsg(c, &msg).unwrap());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ping() {
        assert_eq!(is_ping("a", "a"), true);
        assert_eq!(is_ping("a", "a ^"), true);
        assert_eq!(is_ping("a", "a:"), true);
        assert_eq!(is_ping("a", "a: hi"), true);
        assert_eq!(is_ping("a", "a hi"), true);
        assert_eq!(is_ping("a", "a,"), true);
        assert_eq!(is_ping("a", "a, hi"), true);
        assert_eq!(is_ping("a", "b: a:"), true);
        assert_eq!(is_ping("a", "b, a:"), true);
        assert_eq!(is_ping("a", "b,a:"), true);
        assert_eq!(is_ping("a", "b,a"), true);
        assert_eq!(is_ping("a", "a,b:"), true);
        assert_eq!(is_ping("a", "a,b"), true);
        assert_eq!(is_ping("b", "also, b:"), true);
        assert_eq!(is_ping("b", "also, b: hi"), true);
        assert_eq!(is_ping("a", "words words words a"), true);
        assert_eq!(is_ping("a", "hi, a"), true);
        assert_eq!(is_ping("a", "hi a"), true);
        assert_eq!(is_ping("a", "@a"), true);
        assert_eq!(is_ping("a", "@a:"), true);
        assert_eq!(is_ping("a", "@a: hi"), true);
        assert_eq!(is_ping("a", "@a, hi"), true);
        assert_eq!(is_ping("a", "@a hi"), true);
        assert_eq!(is_ping("a", "...a"), true);
        assert_eq!(is_ping("a", "a... hi"), true);
        assert_eq!(is_ping("a", "b/a:"), true);
        assert_eq!(is_ping("a", "a/b:"), true);
        assert_eq!(is_ping("a", " a:"), true);
    }

    #[test]
    fn test_is_ping_no_partial_nick() {
        assert_eq!(is_ping("a", "abc"), false);
        assert_eq!(is_ping("a", "bac"), false);
        assert_eq!(is_ping("a", "bca"), false);
        assert_eq!(is_ping("a", "abc bac bca"), false);
        assert_eq!(is_ping("a", "lemonades are happy at car parks"), false);
    }

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

        // don't resolve email addresses
        assert!(add_scheme_for_tld("test@gmail.com").is_none());
        assert!(add_scheme_for_tld("word.word@gmail.com").is_none());

        // don't resolve tokens beinning with @
        assert!(add_scheme_for_tld("@gmail.com").is_none());
        assert!(add_scheme_for_tld("@endless.horse").is_none());
    }
}
