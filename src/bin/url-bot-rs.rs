/*
 * url-bot-rs
 *
 * URL parsing IRC bot
 *
 */

use url_bot_rs::VERSION;
use url_bot_rs::sqlite::Database;
use url_bot_rs::config::{
    Rtd,
    Conf,
    find_configs_in_dir,
    ensure_parent_dir,
    load_flattened_configs,
};
use url_bot_rs::message::handle_message;
use url_bot_rs::{feat, param};

use docopt::Docopt;
use failure::Error;
use irc::client::prelude::*;
use std::process;
use std::thread;
use std::time::Duration;
use std::path::PathBuf;
use stderrlog::{Timestamp, ColorChoice};
use atty::{is, Stream};
use directories::ProjectDirs;
use serde_derive::Deserialize;
use log::{info, warn, error};

// docopt usage string
const USAGE: &str = "
URL munching IRC bot.

Usage:
    url-bot-rs [options] [-v...] [--conf=PATH...] [--conf-dir=DIR...]

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

    run(args).unwrap_or_else(|e| {
        error!("{}", e);
        process::exit(1);
    });
}

fn run(args: Args) -> Result<(), Error> {
    // find configs in locations specified on command line
    let mut config_paths: Vec<PathBuf> = get_cli_configs(&args)?;

    // add configurations in default paths
    add_default_configs(&mut config_paths);

    // create defaults for non-existent paths
    create_default_configs(&config_paths)?;

    // create a list of configurations
    let configs: Vec<Conf> = load_flattened_configs(config_paths);

    // threaded instances
    let threads: Vec<_> = configs
        .into_iter()
        .map(|conf| {
            thread::spawn(move || {
                run_instance(conf).unwrap_or_else(|e| {
                    error!("{}", e);
                    process::exit(1);
                });
            })
        })
        .collect();

    for thread in threads {
        thread.join().ok();
    }

    Ok(())
}

/// Add configurations from default sources.
///
/// - valid configuration files under the default search path.
/// - the configuration at the default config path (to be created if
///   non-existing).
fn add_default_configs(configs: &mut Vec<PathBuf>) {
    let dirs = ProjectDirs::from("org", "", "url-bot-rs").unwrap();
    let default_conf_dir = dirs.config_dir();

    if configs.is_empty() {
        if let Ok(dir_confs) = find_configs_in_dir(default_conf_dir) {
            configs.extend(dir_confs)
        }
    }

    if configs.is_empty() {
        let default_conf = default_conf_dir.join("config.toml");
        configs.push(default_conf);
    }
}

/// Combine sources of user-specified configuration file locations
///
/// Get all configurations specified with `--conf`, and all configs found in
/// search paths specified with `--conf-dir`
fn get_cli_configs(args: &Args) -> Result<Vec<PathBuf>, Error> {
    let dir_configs = args.flag_conf_dir
        .iter()
        .try_fold(vec![], |mut result, dir| -> Result<_, Error> {
            let dir_configs = find_configs_in_dir(dir)?;
            result.extend(dir_configs);
            Ok(result)
        })?;

    Ok([&dir_configs[..], &args.flag_conf[..]].concat())
}

/// Create a default valued configuration file, if config path doesn't exist
fn create_default_configs(paths: &[PathBuf]) -> Result<(), Error> {
    for p in paths {
        ensure_parent_dir(p)?;

        // create a default-valued config if it doesn't exist
        if !p.exists() {
            info!(
                "Configuration `{}` doesn't exist, creating default",
                p.to_str().unwrap()
            );
            warn!(
                "You should modify this file to include a useful IRC \
                configuration"
            );
            Conf::default().write(p)?;
        }
    };

    Ok(())
}

/// Run an instance, handling restart if configured.
fn run_instance(conf: Conf) -> Result<(), Error> {
    let net = conf.network.name.clone();

    if let Some(ref path) = conf.path {
        if conf.network.enable {
            info!("[{}] using configuration: {}", net, path.display());
        } else {
            warn!("[{}] ignoring configuration in: {}", net, path.display());
            return Ok(());
        }
    }

    let rtd: Rtd = Rtd::new()
        .conf(conf)
        .load()?
        .init_http_client()?;

    let timeout = param!(rtd, reconnect_timeout);
    let sleep_dur = Duration::from_secs(timeout);

    loop {
        match connect_instance(&rtd) {
            Ok(_) => error!("[{}] disconnected for unknown reason", net),
            Err(e) => error!("[{}] disconnected: {}", net, e),
        };

        if !feat!(rtd, reconnect) {
            break Ok(());
        }

        info!("[{}] reconnecting in {} seconds", net, timeout);
        thread::sleep(sleep_dur);
    }
}

/// Connect to a server and handle IRC messages.
fn connect_instance(rtd: &Rtd) -> Result<(), Error> {
    let mut rtd = rtd.clone();
    let net = &rtd.conf.network.name;

    let db = if let Some(ref path) = rtd.paths.db {
        info!("[{}] using database: {}", net, path.display());
        Database::open(path)?
    } else {
        Database::open_in_memory()?
    };

    if feat!(rtd, history) && rtd.paths.db.is_none() {
        info!("[{}] using in-memory database", net);
    }

    let mut reactor = IrcReactor::new()?;

    let client = reactor.prepare_client_and_connect(&rtd.conf.client)?;
    client.identify()?;

    info!("[{}] connected", net);

    reactor.register_client_with_handler(client, move |client, message| {
        handle_message(client, &message, &mut rtd, &db);
        Ok(())
    });

    reactor.run()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::env;
    use std::path::Path;
    use tempfile::tempdir;
    use url_bot_rs::config::Conf;

    #[test]
    fn test_get_cli_configs() {
        let tmp_dir = tempdir().unwrap();
        let cfg_dir = tmp_dir.path();

        let mut args = Args::default();
        args.flag_conf_dir = vec![cfg_dir.to_path_buf()];

        // dir is empty
        assert_eq!(get_cli_configs(&args).unwrap().len(), 0);

        // add configs to --conf-dir directory
        for i in 1..=10 {
            Conf::default().write(cfg_dir.join(i.to_string() + ".cf")).unwrap();
            assert_eq!(get_cli_configs(&args).unwrap().len(), i);
        }

        // add --conf option
        args.flag_conf.extend(vec![cfg_dir.join("c1.conf")]);
        assert_eq!(get_cli_configs(&args).unwrap().len(), 11);

        // add more --conf options
        for i in 12..=20 {
            args.flag_conf.extend(vec![cfg_dir.join(i.to_string() + ".toml")]);
            assert_eq!(get_cli_configs(&args).unwrap().len(), i);
        }
    }

    #[test]
    fn test_get_cli_configs_failures() {
        // dir doesn't exist
        let mut args = Args::default();
        args.flag_conf_dir = vec![PathBuf::from("/surely/no/way/this/exists")];
        assert!(get_cli_configs(&args).is_err());
    }

    #[test]
    fn test_add_default_configs() {
        let dirs = ProjectDirs::from("org", "", "url-bot-rs").unwrap();
        let default_conf_dir = dirs.config_dir();

        let tmp_dir = tempdir().unwrap();
        let cfg_home = tmp_dir.path();

        // get configuration directory - temp directory on linux, real config
        // directory on other platforms
        let cfg_dir = if cfg!(target_os = "linux") {
            env::set_var("XDG_CONFIG_HOME", &cfg_home.as_os_str());
            cfg_home.join("url-bot-rs")
        } else {
            PathBuf::from(default_conf_dir)
        };

        println!("configuration directory: {}", &cfg_dir.display());

        fs::create_dir_all(&cfg_dir).ok();

        let files_in_dir = fs::read_dir(&cfg_dir)
            .unwrap()
            .flatten()
            .count();

        if files_in_dir > 0 {
            panic!("configuration directory contains files, can't run test");
        }

        test_add_default_configs_default(&cfg_dir);
        test_add_default_configs_dir(&cfg_dir);
        empty_dir(&cfg_dir);
        test_add_default_configs_dir_many(&cfg_dir);
    }

    fn empty_dir(dir: &Path) {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .for_each(|e| fs::remove_file(e).unwrap());
    }

    /// no cli configs provided, no configs in default search path
    /// => default configuration (config.toml)
    fn test_add_default_configs_default(cfg_dir: &Path) {
        let mut configs = vec![];

        add_default_configs(&mut configs);

        println!("{:?}", configs);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0], cfg_dir.join("config.toml"));
    }

    /// no cli configs, config in default search path
    fn test_add_default_configs_dir(cfg_dir: &Path) {
        let mut configs = vec![];

        let conf = cfg_dir.join("a.conf");
        Conf::default().write(&conf).unwrap();
        add_default_configs(&mut configs);

        println!("{:?}", configs);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs, vec![conf]);
    }

    /// no cli configs, multiple configs in default search path
    fn test_add_default_configs_dir_many(cfg_dir: &Path) {
        let mut configs = vec![];

        let conf_a = cfg_dir.join("b.conf");
        let conf_b = cfg_dir.join("c.conf");
        Conf::default().write(&conf_a).unwrap();
        Conf::default().write(&conf_b).unwrap();
        add_default_configs(&mut configs);

        println!("{:?}", configs);
        assert_eq!(configs.len(), 2);
        assert!(configs.contains(&conf_a));
        assert!(configs.contains(&conf_b));
    }
}
