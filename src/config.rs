/*
 * Application configuration
 *
 */
use std::fs;
use toml;
use std::path::{Path, PathBuf};
use irc::client::data::Config as IrcConfig;
use failure::Error;
use std::fmt;
use directories::{ProjectDirs};
use super::Args;

// serde structures defining the configuration file structure
#[derive(Default, Deserialize)]
pub struct Conf {
    pub features: Features,
    #[serde(rename = "parameters")]
    pub params: Parameters,
    pub database: Database,
    #[serde(rename = "connection")]
    pub client: IrcConfig,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Features {
    pub report_metadata: bool,
    pub report_mime: bool,
    pub mask_highlights: bool,
    pub send_notice: bool,
    pub history: bool,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Database {
    pub path: String,
    #[serde(rename = "type")]
    #[serde(default = "d_db_type")]
    pub db_type: String,
}
fn d_db_type() -> String { "sqlite".to_string() }

#[derive(Default, Serialize, Deserialize)]
pub struct Parameters {
    #[serde(default = "d_url_limit")]
    pub url_limit: u8,
    #[serde(default = "d_user_agent")]
    pub user_agent: String,
    #[serde(default = "d_accept_lang")]
    pub accept_lang: String,
}
fn d_url_limit() -> u8 { 10 }
fn d_user_agent() -> String { "Mozilla/5.0".to_string() }
fn d_accept_lang() -> String { "en".to_string() }

impl Conf {
    // load configuration TOML from a file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let conf = fs::read_to_string(path.as_ref())?;
        let conf: Conf = toml::de::from_str(&conf)?;
        Ok(conf)
    }
}

// run time data structure. this is used to pass around mutable runtime data
// where it's needed, including command line arguments, configuration file
// settings, any parameters defined based on both of these sources, and
// any other data used at runtime
#[derive(Default)]
pub struct Rtd {
    // paths
    pub paths: Paths,
    // configuration file data
    pub conf: Conf,
    // command-line arguments
    pub args: Args,
    // settings derived from both CLI args and configuration file
    pub history: bool,
}

#[derive(Default)]
pub struct Paths {
    pub conf: PathBuf,
    pub db: Option<PathBuf>,
}

impl Rtd {
    pub fn from_args(args: Args) -> Result<Self, Error> {
        let mut rtd = Rtd::default();

        // move command line arguments
        rtd.args = args;

        // get a config file path
        let dirs = ProjectDirs::from("org", "", "url-bot-rs").unwrap();
        rtd.paths.conf = match rtd.args.flag_conf {
            // configuration file path specified as command line parameter
            Some(ref cp) => cp.clone(),
            // default path
            _ => dirs.config_dir().join("config.toml")
        };

        // load config file
        rtd.conf = Conf::load(&rtd.paths.conf)?;

        // set database path and history flag
        let (hist_enabled, db_path) = Self::get_db_info(&rtd, &dirs);
        rtd.history = hist_enabled;
        rtd.paths.db = db_path;

        Ok(rtd)
    }

    fn get_db_info(
        rtd: &Rtd, dirs: &ProjectDirs
    ) -> (bool, Option<PathBuf>) {
        if let Some(ref path) = rtd.args.flag_db {
            // enable history when db path given as CLI argument
            (true, Some(PathBuf::from(path)))
        } else if rtd.conf.features.history == false {
            // no path specified on CLI, and history disabled in configuration
            (false, None)
        } else if !rtd.conf.database.path.is_empty() {
            // (non-empty) db path specified in configuration
            (true, Some(PathBuf::from(&rtd.conf.database.path)))
        } else if rtd.conf.database.db_type == "sqlite" {
            // database type is sqlite, but no path given, use default
            (true, Some(dirs.data_local_dir().join("history.db")))
        } else {
            // use in-memory database
            (true, None)
        }
    }
}

// implementation of Display trait for multiple structs above
macro_rules! impl_display {
    ($($t:ty),+) => {
        $(impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}", toml::ser::to_string(self).unwrap())
            }
        })+
    }
}
impl_display!(Features, Parameters, Database);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_example_conf() {
        // test that the example configuration file parses without error
        let mut args = Args::default();
        args.flag_conf = Some(PathBuf::from("example.config.toml"));
        Rtd::from_args(args).unwrap();
    }
}
