/*
 * url-bot-rs
 *
 * URL parsing IRC bot
 *
 */

extern crate irc;
extern crate hyper;
extern crate curl;
extern crate htmlescape;
extern crate regex;

use regex::Regex;
use curl::easy::{Easy2, Handler, WriteError, List};
use irc::client::prelude::*;
use htmlescape::decode_html;

/* Message { tags: None, prefix: Some("edcragg!edcragg@ip"), command: PRIVMSG("#music", "test") } */

fn main() {

	let server = IrcServer::new("config.toml").unwrap();
	server.identify().unwrap();
	server.for_each_incoming(|message| {

		match message.command {

			Command::PRIVMSG(ref target, ref msg) => {

				let tokens: Vec<_> = msg.split_whitespace().collect();

				for t in tokens {
					let mut title = None;

					let url;
					match t.parse::<hyper::Uri>() {
						Ok(u) => { url = u; }
						_     => { continue; }
					}

					match url.scheme() {
						Some("http")  => { title = resolve_url(t); }
						Some("https") => { title = resolve_url(t); }
						_ => ()
					}

					match title {
						Some(s) => {
							server.send_privmsg(
								message.response_target().unwrap_or(target), &s
							).unwrap();
						}
						_ => ()
					}
				}
			}

			_ => (),
		}

	}).unwrap()
}

#[derive(Debug)]
struct Collector(Vec<u8>);

impl Handler for Collector {
	fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
		self.0.extend_from_slice(data);
		Ok(data.len())
	}
}

fn resolve_url(url: &str) -> Option<String> {

	println!("RESOLVE {}", url);

	let mut easy = Easy2::new(Collector(Vec::new()));

	easy.get(true).unwrap();
	easy.url(url).unwrap();
	easy.follow_location(true).unwrap();
	easy.useragent("url-bot-rs/0.1").unwrap();

	let mut headers = List::new();
	headers.append("Accept-Language: en").unwrap();
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

	let mut title_dec = String::new();
	match decode_html(title_enc) {
		Ok(s) => { title_dec = s; }
		_     => ()
	};

	/* strip leading and tailing whitespace from title */
	let re = Regex::new(r"^[\s\n]*(?P<title>.*?)[\s\n]*$").unwrap();
	let res = re.captures(&title_dec).unwrap()["title"].to_string();

	match res.chars().count() {
		0 => None,
		_ => {
			println!("SUCCESS \"{}\"", res);
			Some(res)
		}
	}
}


#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn resolve_urls() {
		assert_ne!(None, resolve_url("https://youtube.com"));
		assert_ne!(None, resolve_url("https://google.co.uk"));
	}

	#[test]
	fn parse_contents() {
		assert_eq!(None, parse_content(&"".to_string()));
		assert_eq!(None, parse_content(&"    ".to_string()));
		assert_eq!(None, parse_content(&"<title></title>".to_string()));
		assert_eq!(None, parse_content(&"<title>    </title>".to_string()));
		assert_eq!(None, parse_content(&"floofynips, not a real webpage".to_string()));
		assert_eq!(Some("cheese is nice".to_string()), parse_content(&"<title>cheese is nice</title>".to_string()));
		assert_eq!(Some("squanch".to_string()), parse_content(&"<title>     squanch</title>".to_string()));
		assert_eq!(Some("squanch".to_string()), parse_content(&"<title>squanch     </title>".to_string()));
		assert_eq!(Some("squanch".to_string()), parse_content(&"<title>\nsquanch</title>".to_string()));
		assert_eq!(Some("squanch".to_string()), parse_content(&"<title>\n  \n  squanch</title>".to_string()));
		assert_eq!(Some("we like the moon".to_string()), parse_content(&"<title>\n  \n  we like the moon</title>".to_string()));
		assert_eq!(Some("&hello123&<>''~".to_string()), parse_content(&"<title>&amp;hello123&amp;&lt;&gt;''~</title>".to_string()));
	}
}
