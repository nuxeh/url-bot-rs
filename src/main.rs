extern crate irc;
extern crate hyper;
extern crate tokio_core;
extern crate curl;

use curl::easy::Easy;
use std::io::{stdout, Write};

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

fn resolve_url(url: &str) {

    let mut dst = Vec::new();
    let mut easy = Easy::new();
    easy.url(url).unwrap();

    let mut transfer = easy.transfer();
    transfer.write_function(|data| {
        dst.extend_from_slice(data);
        Ok(data.len())
    }).unwrap();
    transfer.perform().unwrap();

}
