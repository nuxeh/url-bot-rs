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

use super::buildinfo;

// serde structures defining the configuration file structure
#[derive(Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Features {
    pub report_metadata: bool,
    pub report_mime: bool,
    pub mask_highlights: bool,
    pub send_notice: bool,
    pub history: bool,
}

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone)]
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

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Conf {
    pub features: Features,
    #[serde(rename = "parameters")]
    pub params: Parameters,
    pub database: Database,
    #[serde(rename = "connection")]
    pub client: IrcConfig,
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
#[derive(Default, Clone)]
pub struct Rtd {
    /// paths
    pub paths: Paths,
    /// configuration file data
    pub conf: Conf,
    pub history: bool,
}

#[derive(Default, Clone)]
pub struct Paths {
    pub conf: PathBuf,
    pub db: Option<PathBuf>,
}

impl Rtd {
    pub fn new() -> Self {
        Rtd::default()
    }

    pub fn db(&mut self, path: Option<PathBuf>) -> &mut Self {
        self.paths.db = path;
        self
    }

    pub fn conf(&mut self, path: &Option<PathBuf>) -> &mut Self {
        let dirs = ProjectDirs::from("org", "", "url-bot-rs").unwrap();

        self.paths.conf = match path {
            Some(ref cp) => expand_tilde(cp),
            None => dirs.config_dir().join("config.toml")
        };

        self
    }

    pub fn load(&mut self) -> Result<Self, Error> {
        create_dir_if_missing(self.paths.conf.parent().unwrap())?;

        // create a default-valued config if it doesn't exist
        if !self.paths.conf.exists() {
            info!("Configuration `{}` doesn't exist, creating default",
                self.paths.conf.to_str().unwrap());
            warn!("You should modify this file to include a useful IRC \
                configuration");
            Conf::default().write(&self.paths.conf)?;
        }

        // load config file
        self.conf = Conf::load(&self.paths.conf)?;

        // get db path, and history
        self.set_db_info();

        if let Some(dp) = self.paths.db.clone() {
            create_dir_if_missing(dp.parent().unwrap())?;
        }

        // set url-bot-rs version number in the irc client configuration
        self.conf.client.version = Some(String::from(buildinfo::PKG_VERSION));

        Ok(self.clone())
    }

    fn set_db_info(&mut self) {
        let dirs = ProjectDirs::from("org", "", "url-bot-rs").unwrap();

        let (hist_enabled, db_path) = if let Some(ref path) = self.paths.db {
            // enable history when db path given as CLI argument
            (true, Some(PathBuf::from(path)))
        } else if !self.conf.features.history {
            // no path specified on CLI, and history disabled in configuration
            (false, None)
        } else if !self.conf.database.path.is_empty() {
            // (non-empty) db path specified in configuration
            (true, Some(PathBuf::from(&self.conf.database.path)))
        } else if self.conf.database.db_type == "sqlite" {
            // database type is sqlite, but no path given, use default
            (true, Some(dirs.data_local_dir().join("history.db")))
        } else {
            // use in-memory database
            (true, None)
        };

        self.history = hist_enabled;
        self.paths.db = db_path.and_then(|p| Some(expand_tilde(&p)));
    }
}

/// implementation of Display trait for multiple structs in this module
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
        info!("Directory `{}` doesn't exist, creating it", pdir);
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
    /// test that the example configuration file parses without error
    fn load_example_conf() {
        Rtd::new()
            .conf(&Some(PathBuf::from("example.config.toml")))
            .load()
            .unwrap();
    }

    #[test]
    /// test that the example configuration matches default values
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

        assert_eq!(default, example);
    }

    #[test]
    fn test_expand_tilde() {
        let homedir: PathBuf = BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_owned();

        assert_eq!(expand_tilde(&PathBuf::from("/")),
            PathBuf::from("/"));
        assert_eq!(expand_tilde(&PathBuf::from("/abc/~def/ghi/")),
            PathBuf::from("/abc/~def/ghi/"));
        assert_eq!(expand_tilde(&PathBuf::from("~/")),
            PathBuf::from(format!("{}/", homedir.to_str().unwrap())));
        assert_eq!(expand_tilde(&PathBuf::from("~/ac/df/gi/")),
            PathBuf::from(format!("{}/ac/df/gi/", homedir.to_str().unwrap())));
    }
}
