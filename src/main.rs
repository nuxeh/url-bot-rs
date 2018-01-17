extern crate irc;
extern crate hyper;
extern crate tokio_core;
extern crate curl;

use curl::easy::Easy;
use std::io::{stdout, Write};

use curl::easy::{Easy2, Handler, WriteError};

use irc::client::prelude::*;
use hyper::Client;

/* Message { tags: None, prefix: Some("edcragg!edcragg@ip"), command: PRIVMSG("#music", "test") } */

fn main() {

	let server = IrcServer::new("config.toml").unwrap();
	server.identify().unwrap();
	server.for_each_incoming(|message| {

	match message.command {
			Command::PRIVMSG(ref target, ref msg) => {
				let tokens: Vec<_> = msg.split(' ').collect();

				for t in tokens {
					println!("{:?}", t);

					let url = msg.parse::<hyper::Uri>().unwrap();
					match url.scheme() {
						Some("http") => { resolve_url(msg); }
						Some("https") => { resolve_url(msg); }
						_ => {println!("This example only works with 'http' URLs.");}
					}

					server.send_privmsg(
						message.response_target().unwrap_or(target), msg
					).unwrap();
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

fn resolve_url(url: &str) {

	let mut easy = Easy2::new(Collector(Vec::new()));

	easy.get(true).unwrap();
	easy.url(url).unwrap();
	easy.perform().unwrap();

	let contents = easy.get_ref();
	println!("{}", String::from_utf8_lossy(&contents.0));
}
