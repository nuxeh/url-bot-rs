/*
 * url-bot-rs
 *
 * URL parsing IRC bot
 *
 */

extern crate irc;
extern crate rusqlite;
extern crate docopt;
extern crate hyper;
#[macro_use]
extern crate serde_derive;

use docopt::Docopt;
use irc::client::prelude::*;
use rusqlite::Connection;
use std::process;

mod sqlite;
mod http;

/* docopt usage string */
const USAGE: &'static str = "
URL munching IRC bot.

Usage:
    url-bot-rs [options] [--db=PATH]

Options:
    -h --help       Show this help message.
    -d --db=PATH    Use a sqlite database at PATH.
    -c --conf=PATH  Use configuration file at PATH [default: ./config.toml].
    -l --lang=LANG  Language to request in http headers [default: en]
";

#[derive(Debug, Deserialize, Default)]
struct Args {
    flag_db: String,
    flag_conf: String,
    flag_lang: String,
}

/* Message { tags: None, prefix: Some("edcragg!edcragg@ip"), command: PRIVMSG("#music", "test") } */

fn main() {

    let args: Args = Docopt::new(USAGE)
                     .and_then(|d| d.deserialize())
                     .unwrap_or_else(|e| e.exit());

    let db;
    if !args.flag_db.is_empty()
    {
        /* open the sqlite database for logging */
        /* TODO: get database path from configuration */
        /* TODO: make logging optional */
        println!("Using database at: {}", args.flag_db);
        db = sqlite::create_db(&args.flag_db).unwrap();
    } else {
        db = Connection::open_in_memory().unwrap();
    }

    /* load IRC configuration */
    println!("Using configuration at: {}", args.flag_conf);
    let conf = Config::load(args.flag_conf.clone());
    let config = match conf {
        Ok(c)    => c,
        Err(err) => {
            eprintln!("IRC configuration error: {}", err);
            process::exit(1);
        },
    };

    /* create IRC reactor */
    let mut reactor = IrcReactor::new().unwrap();
    let client = reactor.prepare_client_and_connect(&config).unwrap();
    client.identify().unwrap();

    /* register handler */
    reactor.register_client_with_handler(client, move |client, message| {
        match message.command {
            Command::PRIVMSG(ref target, ref msg) => {

                /* get all the words/tokens, put them into an array */
                let tokens: Vec<_> = msg.split_whitespace().collect();

                for t in tokens
                {
                    let mut title = None;

                    let url;
                    match t.parse::<hyper::Uri>() {
                        Ok(u) => { url = u; }
                        _     => { continue; }
                    }

                    match url.scheme() {
                        Some("http")  => {title = http::resolve_url(t, &args.flag_lang);}
                        Some("https") => {title = http::resolve_url(t, &args.flag_lang);}
                        _ => ()
                    }

                    match title {
                        Some(s) => {
                            /* create a log entry struct */
                            let entry = sqlite::LogEntry {
                                id: 0,
                                title: s.clone(),
                                url: t.clone().to_string(),
                                prefix: &message.prefix.clone().unwrap(),
                                channel: target.to_string(),
                                time_created: "".to_string()
                            };

                            /* check for pre-post */
                            let p = if !args.flag_db.is_empty() {
                                sqlite::check_prepost(&db, &entry)
                            } else {None};

                            let msg = match p {
                                Some(p) => {
                                    format!("⤷ {} → {} {} ({})",
                                            s,
                                            p.time_created,
                                            p.user,
                                            p.channel)
                                },
                                None    => {
                                    /* add log entry to database */
                                    if !args.flag_db.is_empty() {
                                        sqlite::add_log(&db, &entry);
                                    }
                                    format!("⤷ {}", s)
                                }
                            };

                            /* send the IRC response */
                            client.send_privmsg(
                                message.response_target().unwrap_or(target),
                                &msg).unwrap();
                        }
                        _ => ()
                    } /* match title */

                } /* for t in tokens */

            } /* Command::PRIVMSG */
            _ => (),
        } /* match message.command */

        Ok(())
    }); /* reactor.register_client_with_handler */

    reactor.run().unwrap();
}
