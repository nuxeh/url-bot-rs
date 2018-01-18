extern crate irc;
extern crate hyper;
extern crate tokio_core;
extern crate curl;
extern crate htmlescape;

use curl::easy::Easy;
use std::io::{stdout, Write};

use curl::easy::{Easy2, Handler, WriteError};

use irc::client::prelude::*;
use hyper::Client;

use htmlescape::decode_html;

/* Message { tags: None, prefix: Some("edcragg!edcragg@ip"), command: PRIVMSG("#music", "test") } */

fn main() {

	let server = IrcServer::new("config.toml").unwrap();
	server.identify().unwrap();
	server.for_each_incoming(|message| {

		match message.command {

			Command::PRIVMSG(ref target, ref msg) => {

				let tokens: Vec<_> = msg.split(' ').collect();

				for t in tokens {
					let mut title = None;
					let url = msg.parse::<hyper::Uri>().unwrap();

					match url.scheme() {
						Some("http")  => { title = resolve_url(msg); }
						Some("https") => { title = resolve_url(msg); }
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

struct Collector(Vec<u8>);

impl Handler for Collector {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        self.0.extend_from_slice(data);
        Ok(data.len())
    }
}

fn resolve_url(url: &str) -> Option<String> {

	let mut easy = Easy2::new(Collector(Vec::new()));

	easy.get(true).unwrap();
	easy.url(url).unwrap();
	easy.perform().unwrap();

	let contents = easy.get_ref();

	let s = String::from_utf8_lossy(&contents.0);
	let s1: Vec<_> = s.split("<title>").collect();
	let s2: Vec<_> = s1[1].split("</title>").collect();
	let title_enc = s2[0];

	let mut title_dec = String::new();
	match decode_html(title_enc) {
		Err(reason) => { }
		Ok(s)       => { title_dec = s; }
	};

	match title_dec.chars().count() {
		0 => None,
		_ => Some(title_dec.to_string())
	}
}
