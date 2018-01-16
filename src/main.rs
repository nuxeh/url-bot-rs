extern crate irc;

use irc::client::prelude::*;

/* Message { tags: None, prefix: Some("edcragg!edcragg@ip"), command: PRIVMSG("#music", "test") } */

fn main() {
	let server = IrcServer::new("config.toml").unwrap();
	server.identify().unwrap();
	server.for_each_incoming(|message| {
	println!("{:?}", message);
	match message.command {
			Command::PRIVMSG(ref target, ref msg) => {
				if msg.starts_with(server.current_nickname()) {
					let tokens: Vec<_> = msg.split(' ').collect();
					if tokens.len() > 2 {
						let n = tokens[0].len() + tokens[1].len() + 2;
						if let Ok(count) = tokens[1].parse::<u8>() {
							for _ in 0..count {
								server.send_privmsg(
									message.response_target().unwrap_or(target),
									&msg[n..]
								).unwrap();
							}
						}
					}
				}
			}
			_ => (),
		}
	}).unwrap()
}
