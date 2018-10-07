/*
 * url-bot-rs
 *
 * URL parsing IRC bot
 *
 */

extern crate irc;
extern crate rusqlite;
extern crate docopt;
#[macro_use]
extern crate serde_derive;
extern crate itertools;
extern crate regex;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate failure;
extern crate htmlescape;
extern crate time;
extern crate reqwest;
extern crate serde_rusqlite;

use docopt::Docopt;
use irc::client::prelude::*;
use std::process;
use self::sqlite::Database;

mod sqlite;
mod http;

// docopt usage string
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
    flag_db: Option<String>,
    flag_conf: String,
    flag_lang: String,
}

// Message { tags: None, prefix: Some("edcragg!edcragg@ip"), command: PRIVMSG("#music", "test") }

fn main() {
    let args: Args = Docopt::new(USAGE)
                     .and_then(|d| d.deserialize())
                     .unwrap_or_else(|e| e.exit());

    // open the sqlite database for logging
    // TODO: get database path from configuration
    // TODO: make logging optional
    let db = if let Some(ref path) = args.flag_db {
        println!("Using database at: {}", path);
        Database::open(path).unwrap()
    } else {
        println!("Using in-memory database");
        Database::open_in_memory().unwrap()
    };

    // load IRC configuration
    println!("Using configuration at: {}", args.flag_conf);
    let config = Config::load(&args.flag_conf).unwrap_or_else(|err| {
        eprintln!("IRC configuration error: {}", err);
        process::exit(1);
    });

    // create IRC reactor
    let mut reactor = IrcReactor::new().unwrap();
    let client = reactor.prepare_client_and_connect(&config).unwrap();
    client.identify().unwrap();

    // register handler
    reactor.register_client_with_handler(client, move |client, message| {
        handle_message(client, message, &args, &db);
        Ok(())
    });

    reactor.run().unwrap();
}

fn handle_message(client: &IrcClient, message: Message, args: &Args, db: &Database) {
    let (target, msg) = match message.command {
        Command::PRIVMSG(ref target, ref msg) => (target, msg),
        _ => return,
    };

        let user = message.source_nickname().unwrap();

    // look at each space seperated message token
    for token in msg.split_whitespace() {
        // the token must be a valid url
        let url = match token.parse::<reqwest::Url>() {
            Ok(url) => url,
            _ => continue,
        };

        // the schema must be http or https
        if !["http", "https"].contains(&url.scheme()) {
            continue;
        }

        // try to get the title from the url
        let title = match http::resolve_url(token, &args.flag_lang) {
            Ok(title) => title,
            Err(err) => {
                println!("ERROR {:?}", err);
                continue
            },
        };

        // create a log entry struct
        let entry = sqlite::NewLogEntry {
            title: &title,
            url: token,
            user: user,
            channel: target,
        };

        // check for pre-post
        let msg = match db.check_prepost(token) {
            Ok(Some(previous_post)) => {
                format!("⤷ {} → {} {} ({})",
                    title,
                    previous_post.time_created,
                    previous_post.user,
                    previous_post.channel
                )
            },
            Ok(None) => {
                // add new log entry to database
                if let Err(err) = db.add_log(&entry) {
                    eprintln!("SQL error: {}", err);
                }
                format!("⤷ {}", title)
            },
            Err(err) => {
                eprintln!("SQL error: {}", err);
                continue
            },
        };

        // send the IRC response
        let target = message.response_target().unwrap_or(target);
        client.send_notice(target, &msg).unwrap();
    }
}
