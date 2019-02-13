/*
 * Application configuration
 *
 */
use std::fs;
use std::fs::File;
use std::io::Write;
use toml;
use std::path::{Path, PathBuf};
use irc::client::data::Config as IrcConfig;
use failure::Error;
use std::fmt;
use directories::{ProjectDirs, BaseDirs};

use super::Args;
use super::buildinfo;

// serde structures defining the configuration file structure
#[derive(Serialize, Deserialize)]
#[serde(default)]
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

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Database {
    pub path: String,
    #[serde(rename = "type")]
    pub db_type: String,
}
impl Default for Database {
    fn default() -> Self {
        Self {
            path: "".to_string(),
            db_type: "in-memory".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Parameters {
    pub url_limit: u8,
    pub accept_lang: String,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            url_limit: 10,
            accept_lang: "en".to_string()
        }
    }
}

impl Conf {
    // load configuration TOML from a file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let conf = fs::read_to_string(path.as_ref())?;
        let conf: Conf = toml::de::from_str(&conf)?;
        Ok(conf)
    }

    // write configuration to a file
    pub fn write(self, path: impl AsRef<Path>) -> Result<(), Error> {
        let mut file = File::create(path)?;
        file.write_all(toml::ser::to_string(&self)?.as_bytes())?;
        Ok(())
    }
}

impl Default for Conf {
    fn default() -> Self {
        Self {
            features: Features::default(),
            params: Parameters::default(),
            database: Database::default(),
            client: IrcConfig {
                nickname: Some("url-bot-rs".to_string()),
                alt_nicks: Some(vec!["url-bot-rs_".to_string()]),
                nick_password: Some("".to_string()),
                username: Some("url-bot-rs".to_string()),
                realname: Some("url-bot-rs".to_string()),
                server: Some("chat.freenode.net".to_string()),
                port: Some(6697),
                password: Some("".to_string()),
                use_ssl: Some(true),
                encoding: Some("UTF-8".to_string()),
                channels: Some(vec![]),
                user_info: Some("Feed me URLs.".to_string()),
                source: Some("https://github.com/nuxeh/url-bot-rs".to_string()),
                ping_time: Some(180),
                ping_timeout: Some(10),
                burst_window_length: Some(8),
                max_messages_in_burst: Some(15),
                should_ghost: Some(false),
                ..IrcConfig::default()
            }
        }
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
            Some(ref cp) => expand_tilde(cp),
            // default path
            _ => dirs.config_dir().join("config.toml")
        };

        // check if config directory exists, create it if it doesn't
        create_dir_if_missing(rtd.paths.conf.parent().unwrap())?;

        // create a default config if it doesn't exist
        if !rtd.paths.conf.exists() {
            eprintln!(
                "Configuration `{}` doesn't exist, creating default",
                rtd.paths.conf.to_str().unwrap()
            );
            eprintln!(
                "You should modify this file to include a useful IRC configuration"
            );
            Conf::default().write(&rtd.paths.conf)?;
        }

        // load config file
        rtd.conf = Conf::load(&rtd.paths.conf)?;

        // set database path and history flag
        let (hist_enabled, db_path) = Self::get_db_info(&rtd, &dirs);
        rtd.history = hist_enabled;
        rtd.paths.db = db_path.and_then(|p| Some(expand_tilde(&p)));

        // check database path exists, create it if it doesn't
        if let Some(dp) = rtd.paths.db.clone() {
            create_dir_if_missing(dp.parent().unwrap())?;
        }

        // set url-bot-rs version number in the irc client configuration
        rtd.conf.client.version = Some(String::from(buildinfo::PKG_VERSION));

        Ok(rtd)
    }

    fn get_db_info(
        rtd: &Rtd, dirs: &ProjectDirs
    ) -> (bool, Option<PathBuf>) {
        if let Some(ref path) = rtd.args.flag_db {
            // enable history when db path given as CLI argument
            (true, Some(PathBuf::from(path)))
        } else if !rtd.conf.features.history {
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

fn create_dir_if_missing(dir: &Path) -> Result<bool, Error> {
    let pdir = dir.to_str().unwrap();
    let exists = pdir.is_empty() || dir.exists();
    if !exists {
        eprintln!("Directory `{}` doesn't exist, creating it", pdir);
        fs::create_dir_all(dir)?;
    }
    Ok(exists)
}

fn expand_tilde(path: &Path) -> PathBuf {
    match (BaseDirs::new(), path.strip_prefix("~")) {
        (Some(bd), Ok(stripped)) => bd.home_dir().join(stripped),
        _ => path.to_owned(),
    }
}

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

    #[test]
    fn example_conf_data_matches_generated_default_values() {
        let example = fs::read_to_string("example.config.toml").unwrap();
        let default = toml::ser::to_string(&Conf::default()).unwrap();

        // print diff (on failure)
        println!("Configuration diff (- example, + default):");
        for diff in diff::lines(&example, &default) {
            match diff {
                diff::Result::Left(l) => println!("-{}", l),
                diff::Result::Both(l, _) => println!(" {}", l),
                diff::Result::Right(r) => println!("+{}", r)
            }
        }
        assert!(default == example);
    }

    #[test]
    fn test_expand_tilde() {
        let homedir: PathBuf = BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_owned();

        assert_eq!(
            expand_tilde(&PathBuf::from("/")),
            PathBuf::from("/")
        );
        assert_eq!(
            expand_tilde(&PathBuf::from("/abc/~def/ghi/")),
            PathBuf::from("/abc/~def/ghi/")
        );
        assert_eq!(
            expand_tilde(&PathBuf::from("~/")),
            PathBuf::from(format!("{}/", homedir.to_str().unwrap()))
        );
        assert_eq!(
            expand_tilde(&PathBuf::from("~/abc/def/ghi/")),
            PathBuf::from(format!("{}/abc/def/ghi/", homedir.to_str().unwrap()))
        );
    }
}
