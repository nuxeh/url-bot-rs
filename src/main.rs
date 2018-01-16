extern crate irc;
extern crate hyper;

use irc::client::prelude::*;
//use hyper::Client;

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
                        Some("http") => { }
                        Some("https") => { }
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
