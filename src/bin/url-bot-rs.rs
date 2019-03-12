/*
 * url-bot-rs
 *
 * URL parsing IRC bot
 *
 */
extern crate url_bot_rs;

extern crate irc;
extern crate rusqlite;
extern crate docopt;
#[macro_use]
extern crate serde_derive;
extern crate itertools;
extern crate regex;
extern crate lazy_static;
extern crate failure;
extern crate time;
extern crate reqwest;
extern crate cookie;
extern crate image;
extern crate serde_rusqlite;
extern crate mime;
extern crate humansize;
extern crate unicode_segmentation;
extern crate toml;
extern crate directories;
#[macro_use]
extern crate log;
extern crate atty;
extern crate stderrlog;
extern crate scraper;

use url_bot_rs::VERSION;
use url_bot_rs::sqlite::Database;
use url_bot_rs::config::Rtd;
use url_bot_rs::message::handle_message;

use docopt::Docopt;
use irc::client::prelude::*;
use std::process;
use std::path::PathBuf;
use stderrlog::{Timestamp, ColorChoice};
use atty::{is, Stream};

// docopt usage string
const USAGE: &str = "
URL munching IRC bot.

Usage:
    url-bot-rs [options] [-v...] [--db=PATH]

Options:
    -h --help       Show this help message.
    --version       Print version.
    -v --verbose    Show extra information.
    -d --db=PATH    Use a sqlite database at PATH.
    -c --conf=PATH  Use configuration file at PATH.
    -t --timestamp  Force timestamps.
";

#[derive(Debug, Deserialize, Default)]
pub struct Args {
    flag_verbose: usize,
    flag_db: Option<PathBuf>,
    flag_conf: Option<PathBuf>,
    flag_timestamp: bool,
}

const MIN_VERBOSITY: usize = 2;

fn main() {
    // parse command line arguments with docopt
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.version(Some(VERSION.to_string())).deserialize())
        .unwrap_or_else(|e| e.exit());

    // don't output colours or include timestamps on stderr if piped
    let (coloured_output, mut timestamp) = if is(Stream::Stderr) {
        (ColorChoice::Auto, Timestamp::Second)
    } else{
        (ColorChoice::Never, Timestamp::Off)
    };

    if args.flag_timestamp { timestamp = Timestamp::Second };

    // start logger
    stderrlog::new()
        .module(module_path!())
        .modules(vec![
            "url_bot_rs::message",
            "url_bot_rs::config",
            "url_bot_rs::http",
        ])
        .verbosity(args.flag_verbose + MIN_VERBOSITY)
        .timestamp(timestamp)
        .color(coloured_output)
        .init()
        .unwrap();

    // get a run-time configuration data structure
    let rtd: Rtd = Rtd::new()
        .conf(&args.flag_conf)
        .db(args.flag_db)
        .load()
        .unwrap_or_else(|err| {
            error!("Error loading configuration: {}", err);
            process::exit(1);
        });

    info!("Using configuration: {}", rtd.paths.conf.display());
    if args.flag_verbose > 0 {
        println!("\n[features]\n{}", rtd.conf.features);
        println!("[parameters]\n{}", rtd.conf.params);
        println!("[database]\n{}", rtd.conf.database);
    }

    // open the sqlite database for logging
    let db = if let Some(ref path) = rtd.paths.db {
        info!("Using database: {}", path.display());
        Database::open(path).unwrap_or_else(|err| {
            error!("Database error: {}", err);
            process::exit(1);
        })
    } else {
        if rtd.history { info!("Using in-memory database"); }
        Database::open_in_memory().unwrap()
    };

    // create IRC reactor
    let mut reactor = IrcReactor::new().unwrap();
    let client = reactor
        .prepare_client_and_connect(&rtd.conf.client)
        .unwrap_or_else(|err| {
        error!("IRC prepare error: {}", err);
        process::exit(1);
    });
    client.identify().unwrap();

    // register handler
    reactor.register_client_with_handler(client, move |client, message| {
        handle_message(client, &message, &rtd, &db);
        Ok(())
    });

    reactor.run().unwrap_or_else(|err| {
        error!("IRC client error: {}", err);
        process::exit(1);
    });
}
