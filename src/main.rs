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
extern crate directories;

mod sqlite;
mod http;
mod config;
mod message;

pub mod buildinfo {
   include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

use docopt::Docopt;
use irc::client::prelude::*;
use std::process;
use std::path::PathBuf;

use self::sqlite::Database;
use self::config::Rtd;
use self::message::handle_message;

// docopt usage string
const USAGE: &'static str = "
URL munching IRC bot.

Usage:
    url-bot-rs [options] [--db=PATH]

Options:
    -h --help       Show this help message.
    --version       Print version.
    -v --verbose    Show extra information.
    -D --debug      Print debugging information.
    -d --db=PATH    Use a sqlite database at PATH.
    -c --conf=PATH  Use configuration file at PATH.
";

#[derive(Debug, Deserialize, Default)]
pub struct Args {
    flag_verbose: bool,
    flag_debug: bool,
    flag_db: Option<PathBuf>,
    flag_conf: Option<PathBuf>,
}

fn version() -> String {
    format!("v{}{} (build: {})",
        buildinfo::PKG_VERSION,
        buildinfo::GIT_VERSION
            .map_or_else(|| "".to_owned(),
                |v| format!(" (git {})", v)),
        buildinfo::PROFILE,
    )
}

fn main() {
    let args: Args = Docopt::new(USAGE)
                     .and_then(|d| d.version(Some(version())).deserialize())
                     .unwrap_or_else(|e| e.exit());

    let rtd: Rtd = Rtd::from_args(args).unwrap_or_else(|err| {
        eprintln!("Error loading configuration: {}", err);
        process::exit(1);
    });

    println!("Using configuration: {}", rtd.paths.conf.display());
    if rtd.args.flag_verbose {
        println!("\n[features]\n{}", rtd.conf.features);
        println!("[parameters]\n{}", rtd.conf.params);
        println!("[database]\n{}", rtd.conf.database);
    }

    // open the sqlite database for logging
    let db = if let Some(ref path) = rtd.paths.db {
        println!("Using database: {}", path.display());
        Database::open(path).unwrap_or_else(|err| {
            eprintln!("Database error: {}", err);
            process::exit(1);
        })
    } else {
        if rtd.history { println!("Using in-memory database"); }
        Database::open_in_memory().unwrap()
    };

    // create IRC reactor
    let mut reactor = IrcReactor::new().unwrap();
    let client = reactor
        .prepare_client_and_connect(&rtd.conf.client)
        .unwrap_or_else(|err| {
        eprintln!("IRC prepare error: {}", err);
        process::exit(1);
    });
    client.identify().unwrap();

    // register handler
    reactor.register_client_with_handler(client, move |client, message| {
        handle_message(client, message, &rtd, &db);
        Ok(())
    });

    reactor.run().unwrap_or_else(|err| {
        eprintln!("IRC client error: {}", err);
        process::exit(1);
    });
}
