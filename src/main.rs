extern crate irc;

use irc::client::prelude::*;

fn main() {
    let server = IrcServer::new("config.toml").unwrap();
    server.identify().unwrap();
    server.for_each_incoming(|message| {
	println!("{:?}", message);
        // Do message processing.
    }).unwrap()
}
