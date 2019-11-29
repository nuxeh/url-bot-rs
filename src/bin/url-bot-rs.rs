/*
 * url-bot-rs
 *
 * URL parsing IRC bot
 *
 */
extern crate url_bot_rs;

#[macro_use] extern crate log;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate failure;

extern crate irc;
extern crate rusqlite;
extern crate docopt;
extern crate itertools;
extern crate regex;
extern crate lazy_static;
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
extern crate atty;
extern crate stderrlog;
extern crate scraper;

use url_bot_rs::VERSION;
use url_bot_rs::sqlite::Database;
use url_bot_rs::config::{Rtd, find_configs_in_dir};
use url_bot_rs::message::handle_message;

use docopt::Docopt;
use failure::Error;
use irc::client::prelude::*;
use std::process;
use std::thread;
use std::path::PathBuf;
use stderrlog::{Timestamp, ColorChoice};
use atty::{is, Stream};
use directories::ProjectDirs;

// docopt usage string
const USAGE: &str = "
URL munching IRC bot.

Usage:
    url-bot-rs [options] [-v...] [--conf=PATH...]

Options:
    -h --help           Show this help message.
    --version           Print version.
    -v --verbose        Show extra information.
    -c --conf=PATH      Use configuration file(s) at PATH.
    -d --conf-dir=DIR   Search for configuration file(s) in DIR.
    -t --timestamp      Force timestamps.
";

#[derive(Debug, Deserialize, Default)]
pub struct Args {
    flag_verbose: usize,
    flag_conf: Vec<PathBuf>,
    flag_conf_dir: Vec<PathBuf>,
    flag_timestamp: bool,
}

const MIN_VERBOSITY: usize = 2;

fn main() {
    // parse command line arguments with docopt
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.version(Some(VERSION.to_string())).deserialize())
        .unwrap_or_else(|e| e.exit());

    // avoid timestamping when piped, e.g. systemd
    let timestamp = if is(Stream::Stderr) || args.flag_timestamp {
        Timestamp::Second
    } else {
        Timestamp::Off
    };

    stderrlog::new()
        .module(module_path!())
        .modules(vec![
            "url_bot_rs::message",
            "url_bot_rs::config",
            "url_bot_rs::http",
        ])
        .verbosity(args.flag_verbose + MIN_VERBOSITY)
        .timestamp(timestamp)
        .color(ColorChoice::Never)
        .init()
        .unwrap();

    let configs = get_configs(&args).unwrap_or_else(|e| {
        error!("{}", e);
        process::exit(1);
    });

    if configs.is_empty() {
        let dirs = ProjectDirs::from("org", "", "url-bot-rs").unwrap();
        let conf = dirs.config_dir().join("config.toml");
        let db = dirs.data_local_dir().join("history.db");
        run_instance(&conf, Some(&db)).unwrap_or_else(|e| {
            error!("{}", e);
            process::exit(1);
        });
    }

    let threads: Vec<_> = configs
        .into_iter()
        .map(|conf| {
            thread::spawn(move || {
                let conf = conf.clone();
                run_instance(&conf, None).unwrap_or_else(|e| {
                    error!("{}", e);
                    process::exit(1);
                });
            })
        })
        .collect();

    for thread in threads {
        thread.join().ok();
    }
}

fn get_configs(args: &Args) -> Result<Vec<PathBuf>, Error> {
    let dir_configs = args.flag_conf_dir
        .iter()
        .try_fold(vec![], |mut result, dir| -> Result<_, Error> {
            let dir_configs = find_configs_in_dir(dir)?;
            result.extend(dir_configs);
            Ok(result)
        })?;
    Ok([&dir_configs[..], &args.flag_conf[..]].concat())
}

fn run_instance(conf: &PathBuf, db: Option<&PathBuf>) -> Result<(), Error> {
    let mut rtd: Rtd = Rtd::new()
        .conf(conf)
        .db(db)
        .load()?;

    if rtd.conf.network.enable {
        info!("using configuration: {}", conf.display());
    } else {
        return Ok(());
    }

    let db = if let Some(ref path) = rtd.paths.db {
        info!("using database: {}", path.display());
        Database::open(path)?
    } else {
        Database::open_in_memory()?
    };

    if rtd.conf.features.history && rtd.paths.db.is_none() {
        info!("using in-memory database");
    }

    let mut reactor = IrcReactor::new()?;

    let client = reactor.prepare_client_and_connect(&rtd.conf.client)?;
    client.identify()?;

    reactor.register_client_with_handler(client, move |client, message| {
        handle_message(client, &message, &mut rtd, &db);
        Ok(())
    });

    reactor.run()?;

    Ok(())
}
