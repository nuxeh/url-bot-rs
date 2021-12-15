use std::{
    process,
    fs,
    fs::File,
    io::Write,
};
use structopt::StructOpt;
use stderrlog::{Timestamp, ColorChoice};
use atty::{is, Stream};
use log::error;
use failure::{Error, bail};
use reqwest::Url;

use url_bot_rs::{
    feat,
    config::{Rtd, Http},
    http::{RetrieverBuilder, get_title},
    message::add_scheme_for_tld,
    plugins::{TITLE_PLUGINS, PluginConfig},
    cli::url_bot_get::Args
};

const MIN_VERBOSITY: usize = 2;

fn main() {
    // parse command line arguments with structopt
    let args = Args::from_args();

    // don't output colours on stderr if piped
    let coloured_output = if is(Stream::Stderr) {
        ColorChoice::Auto
    } else {
        ColorChoice::Never
    };

    stderrlog::new()
        .module(module_path!())
        .modules(vec![
            "url_bot_rs::config",
            "url_bot_rs::http",
        ])
        .timestamp(Timestamp::Off)
        .quiet(args.quiet)
        .verbosity(args.verbose + MIN_VERBOSITY)
        .color(coloured_output)
        .init()
        .unwrap();

    if let Err(e) = run(&args) {
        error!("{}", e);
    }
}

fn run(args: &Args) -> Result<(), Error> {
    if args.generate {
        generate_plugin_config(args)?;
    } else if let Some(p) = &args.plugin {
        run_plugin(args, p)?;
    } else if args.plugins {
        list_plugins();
    } else {
        scrape_title(args)?;
    }

    Ok(())
}

/// Generate a default plugins configuration file
///
/// The contents depend on currently available plugins, but may include, for
/// example, API keys.
fn generate_plugin_config(args: &Args) -> Result<(), Error> {
    if let Some(path) = &args.conf {
        let mut file = File::create(path)?;
        file.write_all(toml::ser::to_string(&PluginConfig::default())?.as_bytes())?;
        Ok(())
    } else {
        bail!("No configuration file path provided to write to")
    }
}

/// List currently available plugins
fn list_plugins() {
    TITLE_PLUGINS
        .iter()
        .for_each(|p| println!("{}", p.name()));
}

/// Run a named title plugin
fn run_plugin(args: &Args, name: &str) -> Result<(), Error> {
    // Read plugin configuration from TOML
    let path = match &args.conf {
        Some(p) => p,
        _ => bail!("No plugin configuration file provided"),
    };
    let conf = fs::read_to_string(path)?;
    let conf: PluginConfig = toml::de::from_str(&conf)?;

    let url = args.url.parse::<Url>()?;

    let mut rtd = Rtd::new()
        .init_http_client()?;

    rtd.conf.plugins = conf.clone();

    TITLE_PLUGINS
        .iter()
        .filter(|p| p.name() == name)
        .for_each(|p| {
            println!("plugin:   {}",   p.name());
            println!("check:    {}",   p.check(&conf, &url));
            println!("evaluate: {:?}", p.evaluate(&rtd, &url));
        });

    Ok(())
}

/// Scrape a web page for its title
fn scrape_title(args: &Args) -> Result<(), Error> {
    let conf = Http::default();
    let token = add_scheme_for_tld(&args.url).unwrap_or_else(|| args.url.clone());
    let user_agent = args.user_agent.as_ref()
        .or_else(|| conf.user_agent.as_ref());

    let mut builder = RetrieverBuilder::new()
        .retry(
            args.retries.unwrap_or(conf.max_retries).into(),
            args.retry_delay.unwrap_or(conf.retry_delay_s)
        )
        .timeout(args.timeout.unwrap_or(conf.timeout_s))
        .redirect_limit(args.redirect.unwrap_or(conf.max_redirections).into())
        .accept_lang(args.accept_lang.as_ref().unwrap_or(&conf.accept_lang));

    if let Some(v) = user_agent {
        builder = builder.user_agent(v);
    }

    let mut resp = builder
        .build()?
        .request(&token)
        .unwrap_or_else(|err| {
            error!("Error making request: {}", err);
            process::exit(1);
        });

    let mut rtd: Rtd = Rtd::default();
    feat!(rtd, report_metadata) = args.metadata;
    feat!(rtd, report_mime) = args.mime;

    let ret = match get_title(&mut resp, &rtd, args.curl) {
        Ok(t) => {
            if !args.curl { println!("{}", t) };
            0
        },
        Err(e) => {
            error!("Error getting title: {}", e);
            1
        },
    };

    process::exit(ret);
}
