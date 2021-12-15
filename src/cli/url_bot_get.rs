use crate::VERSION;
use std::path::PathBuf;
use structopt::StructOpt;

const ABOUT: &str = "\
URL munching IRC bot, web page title fetching tool.

Retrieve the title or some content from web addresses, primarily a debugging
tool for `url-bot-rs`.
";

const EXAMPLES: &str = "\
EXAMPLES:
    url-bot-get https://google.com
    url-bot-get --conf plugins.toml --generate
    url-bot-get --conf plugins.toml --plugin imgur <url>
";

#[derive(Debug, Default, Clone, StructOpt)]
#[structopt(
    name = "url-bot-get",
    about = ABOUT,
    version = VERSION.as_str(),
    after_help = EXAMPLES,
)]
pub struct Args {
    /// Show extra information.
    #[structopt(short, long, parse(from_occurrences))]
    pub verbose: usize,

    /// Quiet.
    #[structopt(short, long)]
    pub quiet: bool,

    /// The URL to retrieve.
    pub url: String,

    /// Specify user-agent.
    #[structopt(short, long)]
    pub user_agent: Option<String>,

    /// Specify accept-lang.
    #[structopt(short = "l", long)]
    pub accept_lang: Option<String>,

    /// Specify request timeout.
    #[structopt(short, long)]
    pub timeout: Option<u64>,

    /// Specify redirection limit.
    #[structopt(short, long)]
    pub redirect: Option<u8>,

    /// Enable mime reporting.
    #[structopt(long)]
    pub metadata: bool,

    /// Enable mime reporting.
    #[structopt(long)]
    pub mime: bool,

    /// Behave like curl, post page content to stdout.
    #[structopt(long)]
    pub curl: bool,

    /// List available plugins.
    #[structopt(long)]
    pub plugins: bool,

    /// Provide a plugin configuration file.
    #[structopt(long)]
    pub conf: Option<PathBuf>,

    /// Generate a template plugin configuration.
    #[structopt(long)]
    pub generate: bool,

    /// Run named plugin.
    #[structopt(long)]
    pub plugin: Option<String>,

    /// Specify retry limit.
    #[structopt(short = "R", long)]
    pub retries: Option<u8>,

    /// Specify redirection limit.
    #[structopt(short = "T", long)]
    pub retry_delay: Option<u64>,
}
