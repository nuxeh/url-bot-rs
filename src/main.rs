extern crate irc;
extern crate hyper;
extern crate tokio_core;

use std::io::{self, Write};
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
                        Some("http") => { resolve_url(url); }
                        Some("https") => { resolve_url(url); }
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

fn resolve_url(url: hyper::Uri) {

    let mut core = tokio_core::reactor::Core::new().unwrap();
    let handle = core.handle();
    let client = Client::new(&handle);

    let work = client.get(url).and_then(|res| {
        println!("Response: {}", res.status());
        println!("Headers: \n{}", res.headers());

        res.body().for_each(|chunk| {
            io::stdout().write_all(&chunk).map_err(From::from)
        })
    }).map(|_| {
        println!("\n\nDone.");
    });

    core.run(work).unwrap();
}
