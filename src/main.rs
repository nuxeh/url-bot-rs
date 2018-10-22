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
extern crate image;
extern crate serde_rusqlite;
extern crate mime;
extern crate humansize;
extern crate unicode_segmentation;
extern crate toml;

mod sqlite;
mod http;
mod config;
mod message;

use docopt::Docopt;
use irc::client::prelude::*;
use std::process;
use self::sqlite::Database;

use config::Conf;
use message::handle_message;

// docopt usage string
const USAGE: &'static str = "
URL munching IRC bot.

Usage:
    url-bot-rs [options] [--db=PATH]

Options:
    -h --help       Show this help message.
    -v --verbose    Show extra information.
    -d --db=PATH    Use a sqlite database at PATH.
    -c --conf=PATH  Use configuration file at PATH [default: ./config.toml].
    -l --lang=LANG  Language to request in http headers [default: en]
";

#[derive(Debug, Deserialize, Default)]
pub struct Args {
    flag_verbose: bool,
    flag_db: Option<String>,
    flag_conf: String,
    flag_lang: String,
}

// Message { tags: None, prefix: Some("edcragg!edcragg@ip"), command: PRIVMSG("#music", "test") }

fn main() {
    let args: Args = Docopt::new(USAGE)
                     .and_then(|d| d.deserialize())
                     .unwrap_or_else(|e| e.exit());

    println!("Using configuration at: {}", args.flag_conf);

    let conf: Conf = Conf::load(&args.flag_conf).unwrap_or_else(|e| {
        eprintln!("Error loading configuration: {}", e);
        process::exit(1);
    });
    if args.flag_verbose { println!("\n{}", conf.features); }

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

    // create IRC reactor
    let mut reactor = IrcReactor::new().unwrap();
    let client = reactor
        .prepare_client_and_connect(&conf.client)
        .unwrap_or_else(|err| {
        eprintln!("IRC prepare error: {}", err);
        process::exit(1);
    });
    client.identify().unwrap();

    // register handler
    reactor.register_client_with_handler(client, move |client, message| {
        handle_message(client, message, &args, &conf, &db);
        Ok(())
    });

    reactor.run().unwrap_or_else(|err| {
        eprintln!("IRC client error: {}", err);
        process::exit(1);
    });
}
