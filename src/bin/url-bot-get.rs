const USAGE: &str = "
URL munching IRC bot, web page title fetching tool.

Retrieve the title or some content from web addresses, primarily a debugging
tool for `url-bot-rs`.

Usage:
    url-bot-get [options] [-v...] [<url>]

Options:
    -h --help                     Show this help message.
    --version                     Print version.
    -v --verbose                  Show extra information.
    -q --quiet                    Quiet.
    -u=<val> --user-agent=<val>   Specify user-agent.
    -l=<val> --accept-lang=<val>  Specify accept-lang.
    -t=<val> --timeout=<val>      Specify request timeout.
    -r=<val> --redirect=<val>     Specify redirection limit.
    -R=<val> --retries=<val>      Specify retry limit.
    -T=<val> --retry-delay=<val>  Specify redirection limit.
    --metadata=<val>              Enable metadata [default: true].
    --mime=<val>                  Enable mime reporting [default: true].
    --curl                        Behave like curl, post page content to stdout.
    --plugins                     List available plugins.
    --conf=<path>                 Provide a plugin configuration file.
    --generate                    Generate a template plugin configuration.
    --plugin=<name>               Run named plugin.

Examples:
    url-bot-get https://google.com
    url-bot-get --conf plugins.toml --generate
    url-bot-get --conf plugins.toml --plugin imgur <url>
";

#[derive(Debug, Deserialize, Default, Clone)]
pub struct Args {
    flag_verbose: usize,
    flag_quiet: bool,
    arg_url: String,
    flag_user_agent: Option<String>,
    flag_accept_lang: Option<String>,
    flag_timeout: Option<u64>,
    flag_redirect: Option<u8>,
    flag_metadata: bool,
    flag_mime: bool,
    flag_curl: bool,
    flag_plugins: bool,
    flag_conf: Option<PathBuf>,
    flag_generate: bool,
    flag_plugin: Option<String>,
    flag_retries: Option<u8>,
    flag_retry_delay: Option<u64>,
}

use std::{
    process,
    fs,
    fs::File,
    io::Write,
    path::PathBuf,
};
use docopt::Docopt;
use stderrlog::{Timestamp, ColorChoice};
use atty::{is, Stream};
use serde_derive::Deserialize;
use log::error;
use failure::{Error, bail};
use reqwest::Url;

use url_bot_rs::{
    VERSION, feat,
    config::{Rtd, Http},
    http::{RetrieverBuilder, get_title},
    message::add_scheme_for_tld,
    plugins::{TITLE_PLUGINS, PluginConfig},
};

const MIN_VERBOSITY: usize = 2;

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.version(Some(VERSION.to_string())).deserialize())
        .unwrap_or_else(|e| e.exit());

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
        .quiet(args.flag_quiet)
        .verbosity(args.flag_verbose + MIN_VERBOSITY)
        .color(coloured_output)
        .init()
        .unwrap();

    if let Err(e) = run(&args) {
        error!("{}", e);
    }
}

fn run(args: &Args) -> Result<(), Error> {
    if args.flag_generate {
        generate_plugin_config(args)?;
    } else if let Some(p) = &args.flag_plugin {
        run_plugin(args, p)?;
    } else if args.flag_plugins {
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
    if let Some(path) = &args.flag_conf {
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
    let path = match &args.flag_conf {
        Some(p) => p,
        _ => bail!("No plugin configuration file provided"),
    };
    let conf = fs::read_to_string(path)?;
    let conf: PluginConfig = toml::de::from_str(&conf)?;

    let url = args.arg_url.parse::<Url>()?;

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
    let token = add_scheme_for_tld(&args.arg_url).unwrap_or_else(|| args.arg_url.clone());
    let user_agent = args.flag_user_agent.as_ref()
        .or_else(|| conf.user_agent.as_ref());

    let mut builder = RetrieverBuilder::new()
        .retry(
            args.flag_retries.unwrap_or(conf.max_retries).into(),
            args.flag_retry_delay.unwrap_or(conf.retry_delay_s)
        )
        .timeout(args.flag_timeout.unwrap_or(conf.timeout_s))
        .redirect_limit(args.flag_redirect.unwrap_or(conf.max_redirections).into())
        .accept_lang(&args.flag_accept_lang.as_ref().unwrap_or(&conf.accept_lang));

    if let Some(v) = user_agent {
        builder = builder.user_agent(&v);
    }

    let mut resp = builder
        .build()?
        .request(&token)
        .unwrap_or_else(|err| {
            error!("Error making request: {}", err);
            process::exit(1);
        });

    let mut rtd: Rtd = Rtd::default();
    feat!(rtd, report_metadata) = args.flag_metadata;
    feat!(rtd, report_mime) = args.flag_mime;

    let ret = match get_title(&mut resp, &rtd, args.flag_curl) {
        Ok(t) => {
            if !args.flag_curl { println!("{}", t) };
            0
        },
        Err(e) => {
            error!("Error getting title: {}", e);
            1
        },
    };

    process::exit(ret);
}
