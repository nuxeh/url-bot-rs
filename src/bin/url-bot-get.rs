const USAGE: &str = "
URL munching IRC bot, web page title fetching tool.

Retrieve the title or some content from web addresses, a debugging
tool for `url-bot-rs`.

Usage:
    url-bot-rs [options] [-v...] [<url>]

Options:
    -h --help                     Show this help message.
    --version                     Print version.
    -v --verbose                  Show extra information.
    -q --quiet                    Quiet.
    -u=<val> --user-agent=<val>   Specify user-agent.
    -l=<val> --accept-lang=<val>  Specify accept-lang.
    -t=<val> --timeout=<val>      Specify request timeout.
    -r=<val> --redirect=<val>     Specify redirection limit.
    --metadata=<val>              Enable metadata [default: true].
    --mime=<val>                  Enable mime reporting [default: true].
    --curl                        Behave like curl, post page content to stdout.
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
}

extern crate url_bot_rs;

#[macro_use]
extern crate serde_derive;
extern crate lazy_static;
extern crate htmlescape;
extern crate itertools;
extern crate regex;
extern crate failure;
extern crate reqwest;
extern crate cookie;
extern crate image;
extern crate mime;
extern crate humansize;
extern crate irc;
extern crate directories;
extern crate toml;
extern crate docopt;
#[macro_use]
extern crate log;
extern crate atty;
extern crate stderrlog;

use url_bot_rs::config;
use url_bot_rs::http;
use url_bot_rs::VERSION;

use http::{Session, get_title};
use config::Rtd;
use docopt::Docopt;

use stderrlog::{Timestamp, ColorChoice};
use atty::{is, Stream};
use std::process;

const MIN_VERBOSITY: usize = 2;

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.version(Some(VERSION.to_string())).deserialize())
        .unwrap_or_else(|e| e.exit());

    // don't output colours on stderr if piped
    let coloured_output = if is(Stream::Stderr) {
        ColorChoice::Auto
    } else{
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

    let mut rtd: Rtd = Rtd::default();
    rtd.conf.features.report_metadata = args.flag_metadata;
    rtd.conf.features.report_mime = args.flag_mime;

    let mut session = Session::new();

    if let Some(v) = args.flag_timeout {
        info!("overriding timeout to {}s", v);
        session.params.timeout_s = v;
    }
    if let Some(v) = args.flag_redirect {
        info!("overriding redirect limit to {}", v);
        session.params.redirect_limit = v;
    }
    if let Some(v) = args.flag_user_agent {
        info!("overriding user-agent to \"{}\"", v);
        session.params.user_agent = v;
    }
    if let Some(v) = args.flag_accept_lang {
        info!("overriding accept-lang to \"{}\"", v);
        session.accept_lang(&v);
    }

    let mut resp = session
        .request(&args.arg_url)
        .unwrap_or_else(|err| {
            error!("Error making request: {}", err);
            process::exit(1);
        });

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
