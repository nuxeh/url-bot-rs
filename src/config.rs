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
use directories::{BaseDirs, ProjectDirs};

use super::VERSION;

#[derive(Serialize, Deserialize, Clone)]
pub struct Network {
    pub name: String,
    pub enable: bool,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            name: "default".into(),
            enable: true,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Features {
    pub report_metadata: bool,
    pub report_mime: bool,
    pub mask_highlights: bool,
    pub send_notice: bool,
    pub history: bool,
    pub invite: bool,
    pub autosave: bool,
    pub send_errors_to_poster: bool,
    pub reply_with_errors: bool,
    pub partial_urls: bool,
    pub nick_response: bool,
    pub reconnect: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum DbType {
    InMemory,
    Sqlite,
}

impl Default for DbType {
    fn default() -> Self {
        Self::InMemory
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Database {
    #[serde(rename = "type")]
    pub db_type: DbType,
    pub path: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Parameters {
    pub url_limit: u8,
    pub accept_lang: String,
    pub status_channels: Vec<String>,
    pub nick_response_str: String,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            url_limit: 10,
            accept_lang: "en".to_string(),
            status_channels: vec![],
            nick_response_str: "".to_string()
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Conf {
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub features: Features,
    #[serde(default, rename = "parameters")]
    pub params: Parameters,
    #[serde(default)]
    pub database: Database,
    #[serde(rename = "connection")]
    pub client: IrcConfig,
}

impl Conf {
    /// load configuration TOML from a file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let conf = fs::read_to_string(path.as_ref())?;
        let conf: Conf = toml::de::from_str(&conf)?;
        Ok(conf)
    }

    /// write configuration to a file
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        let mut file = File::create(path)?;
        file.write_all(toml::ser::to_string(&self)?.as_bytes())?;
        Ok(())
    }

    /// add an IRC channel to the list of channels in the configuration
    pub fn add_channel(&mut self, name: String) {
        if let Some(ref mut c) = self.client.channels {
            if !c.contains(&name) {
                c.push(name);
            }
        }
    }

    /// remove an IRC channel from the list of channels in the configuration
    pub fn remove_channel(&mut self, name: &str) {
        if let Some(ref mut c) = self.client.channels {
            if let Some(index) = c.iter().position(|c| c == name) {
                c.remove(index);
            }
        }
    }
}

impl Default for Conf {
    fn default() -> Self {
        Self {
            network: Network::default(),
            features: Features::default(),
            params: Parameters::default(),
            database: Database::default(),
            client: IrcConfig {
                nickname: Some("url-bot-rs".to_string()),
                alt_nicks: Some(vec!["url-bot-rs_".to_string()]),
                nick_password: Some("".to_string()),
                username: Some("url-bot-rs".to_string()),
                realname: Some("url-bot-rs".to_string()),
                server: Some("127.0.0.1".to_string()),
                port: Some(6667),
                password: Some("".to_string()),
                use_ssl: Some(false),
                channels: Some(vec!["#url-bot-rs".to_string()]),
                user_info: Some("Feed me URLs.".to_string()),
                ..IrcConfig::default()
            }
        }
    }
}

#[derive(Default, Clone)]
pub struct Rtd {
    /// paths
    pub paths: Paths,
    /// configuration file data
    pub conf: Conf,
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

    pub fn conf(&mut self, path: &PathBuf) -> &mut Self {
        self.paths.conf = expand_tilde(path);
        self
    }

    pub fn db(&mut self, path: Option<&PathBuf>) -> &mut Self {
        self.paths.db = path.map(|p| expand_tilde(p));
        self
    }

    pub fn load(&mut self) -> Result<Self, Error> {
        ensure_parent_dir(&self.paths.conf)?;

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

        self.paths.db = self.get_db_path().map(|p| expand_tilde(&p));

        if let Some(dp) = &self.paths.db {
            ensure_parent_dir(dp)?;
        }

        // set url-bot-rs version number in the irc client configuration
        self.conf.client.version = Some(VERSION.to_string());

        Ok(self.clone())
    }

    fn get_db_path(&mut self) -> Option<PathBuf> {
        if self.conf.features.history {
            match self.conf.database.db_type {
                DbType::InMemory => None,
                DbType::Sqlite => self.get_sqlite_path(),
            }
        } else {
            None
        }
    }

    fn get_sqlite_path(&self) -> Option<PathBuf> {
        let mut path = if let Some(p) = &self.conf.database.path {
            if p.is_empty() {
                None
            } else {
                Some(p.into())
            }
        } else if let Some(p) = &self.paths.db {
            Some(p.into())
        } else {
            None
        };

        if path.is_none() {
            // generate and use a default database path
            let dirs = ProjectDirs::from("org", "", "url-bot-rs").unwrap();
            let db = format!("history.{}.db", self.conf.network.name);
            let db = dirs.data_local_dir().join(&db);
            path = Some(db);
        };

        path
    }
}

fn ensure_parent_dir(file: &Path) -> Result<bool, Error> {
    let without_path = file.components().count() == 1;

    match file.parent() {
        Some(dir) if !without_path => {
            let create = !dir.exists();
            if create {
                info!(
                    "directory `{}` doesn't exist, creating it", dir.display()
                );
                fs::create_dir_all(dir)?;
            }
            Ok(create)
        },
        _ => Ok(false),
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    match (BaseDirs::new(), path.strip_prefix("~")) {
        (Some(bd), Ok(stripped)) => bd.home_dir().join(stripped),
        _ => path.to_owned(),
    }
}

/// non-recursively search for configuration files in a directory
pub fn find_configs_in_dir(dir: &Path) -> Result<impl Iterator<Item = PathBuf>, Error> {
    Ok(fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|e| !e.is_dir() && Conf::load(e).is_ok())
        .take(32))
}

#[cfg(test)]
mod tests {
    extern crate tempfile;

    use super::*;
    use self::tempfile::tempdir;
    use std::env;
    use std::iter;
    use std::panic;

    #[test]
    /// test that the example configuration file parses without error
    fn load_example_conf() {
        Rtd::new()
            .conf(&"example.config.toml".into())
            .load()
            .unwrap();
    }

    #[test]
    fn load_write_default() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("config.toml");

        Rtd::new()
            .conf(&cfg_path)
            .load()
            .unwrap();

        let example = fs::read_to_string("example.config.toml").unwrap();
        let written = fs::read_to_string(cfg_path).unwrap();
        assert_eq!(example, written);
    }

    #[test]
    fn sqlite_path_explicit() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("config.toml");
        let db_path = tmp_dir.path().join("test.db");

        let mut cfg = Conf::default();
        cfg.features.history = true;
        cfg.database.db_type = DbType::Sqlite;
        cfg.write(&cfg_path).unwrap();

        let rtd = Rtd::new()
            .conf(&cfg_path)
            .db(Some(&db_path))
            .load()
            .unwrap();

        assert_eq!(rtd.paths.db, Some(db_path));
    }

    #[test]
    fn sqlite_path_config_overrides_explicit() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("config.toml");
        let db_path = tmp_dir.path().join("test.db");
        let db_path_cfg = tmp_dir.path().join("cfg.test.db");

        let mut cfg = Conf::default();
        cfg.features.history = true;
        cfg.database.db_type = DbType::Sqlite;
        cfg.database.path = Some(db_path_cfg.to_str().unwrap().to_string());
        cfg.write(&cfg_path).unwrap();

        let rtd = Rtd::new()
            .conf(&cfg_path)
            .db(Some(&db_path))
            .load()
            .unwrap();

        assert_eq!(rtd.paths.db, Some(db_path_cfg));
    }

    #[test]
    fn sqlite_path_default() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("config.toml");

        let mut cfg = Conf::default();
        cfg.features.history = true;
        cfg.database.db_type = DbType::Sqlite;
        cfg.write(&cfg_path).unwrap();

        let rtd = Rtd::new()
            .conf(&cfg_path)
            .load()
            .unwrap();

        let dirs = ProjectDirs::from("org", "", "url-bot-rs").unwrap();
        let default = dirs.data_local_dir().join("history.default.db");
        println!("database path: {}", default.to_str().unwrap());
        assert_eq!(rtd.paths.db, Some(default));

        cfg.network.name = "test_net".to_string();
        cfg.write(&cfg_path).unwrap();

        let rtd = Rtd::new()
            .conf(&cfg_path)
            .load()
            .unwrap();

        let default = dirs.data_local_dir().join("history.test_net.db");
        println!("database path: {}", default.to_str().unwrap());
        assert_eq!(rtd.paths.db, Some(default.clone()));

        cfg.database.path = Some("".to_string());
        cfg.write(&cfg_path).unwrap();

        let rtd = Rtd::new()
            .conf(&cfg_path)
            .load()
            .unwrap();

        assert_eq!(rtd.paths.db, Some(default));
    }

    #[test]
    fn sqlite_path_config_overrides_default() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("config.toml");
        let db_path_cfg = tmp_dir.path().join("cfg.test.db");

        let mut cfg = Conf::default();
        cfg.features.history = true;
        cfg.database.db_type = DbType::Sqlite;
        cfg.database.path = Some(db_path_cfg.to_str().unwrap().to_string());
        cfg.write(&cfg_path).unwrap();

        let rtd = Rtd::new()
            .conf(&cfg_path)
            .load()
            .unwrap();

        assert_eq!(rtd.paths.db, Some(db_path_cfg));
    }

    #[test]
    fn test_ensure_parent() {
        let tmp_dir = tempdir().unwrap();
        let tmp_path = tmp_dir.path().join("test/test.file");

        assert_eq!(ensure_parent_dir(&tmp_path).unwrap(), true);
        assert_eq!(ensure_parent_dir(&tmp_path).unwrap(), false);
        assert_eq!(ensure_parent_dir(&tmp_path).unwrap(), false);
    }

    #[test]
    /// CWD should always exist, so don't try to create it
    fn test_ensure_parent_file_in_cwd() {
        assert_eq!(ensure_parent_dir(Path::new("test.f")).unwrap(), false);
        assert_eq!(ensure_parent_dir(Path::new("./test.f")).unwrap(), false);
    }

    #[test]
    fn test_ensure_parent_relative() {
        let tmp_dir = tempdir().unwrap();
        let test_dir = tmp_dir.path().join("subdir");
        println!("creating temp path: {}", test_dir.display());
        fs::create_dir_all(&test_dir).unwrap();

        let cwd = env::current_dir().unwrap();
        env::set_current_dir(test_dir).unwrap();

        let result = panic::catch_unwind(|| {
            assert_eq!(ensure_parent_dir(Path::new("../dir/file")).unwrap(), true);
            assert_eq!(ensure_parent_dir(Path::new("../dir/file")).unwrap(), false);
            assert_eq!(ensure_parent_dir(Path::new("./dir/file")).unwrap(), true);
            assert_eq!(ensure_parent_dir(Path::new("./dir/file")).unwrap(), false);
            assert_eq!(ensure_parent_dir(Path::new("dir2/file")).unwrap(), true);
            assert_eq!(ensure_parent_dir(Path::new("dir2/file")).unwrap(), false);
            assert_eq!(ensure_parent_dir(Path::new("./dir3/file")).unwrap(), true);
            assert_eq!(ensure_parent_dir(Path::new("dir3/file2")).unwrap(), false);
        });

        env::set_current_dir(cwd).unwrap();
        assert!(result.is_ok());
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
    fn conf_add_remove_channel() {
        let mut rtd = Rtd::default();
        check_channels(&rtd, "#url-bot-rs", 1);

        rtd.conf.add_channel("#cheese".to_string());
        check_channels(&rtd, "#cheese", 2);

        rtd.conf.add_channel("#cheese-2".to_string());
        check_channels(&rtd, "#cheese-2", 3);

        rtd.conf.remove_channel(&"#cheese-2".to_string());
        let c = rtd.conf.client.channels.clone().unwrap();

        assert!(!c.contains(&"#cheese-2".to_string()));
        assert_eq!(2, c.len());
    }

    fn check_channels(rtd: &Rtd, contains: &str, len: usize) {
        let c = rtd.conf.client.channels.clone().unwrap();
        println!("{:?}", c);

        assert!(c.contains(&contains.to_string()));
        assert_eq!(len, c.len());
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

    fn write_n_configs(n: usize, dir: &Path) {
        iter::repeat(dir)
            .take(n)
            .enumerate()
            .map(|(i, p)| p.join(i.to_string() + ".conf"))
            .for_each(|p| Conf::default().write(p).unwrap());
    }

    #[test]
    fn test_find_configs_in_dir() {
        let tmp_dir = tempdir().unwrap();
        let cfg_dir = tmp_dir.path();

        assert_eq!(find_configs_in_dir(cfg_dir).unwrap().count(), 0);

        write_n_configs(10, cfg_dir);
        assert_eq!(find_configs_in_dir(cfg_dir).unwrap().count(), 10);

        let mut f = File::create(cfg_dir.join("fake.conf")).unwrap();
        f.write_all(b"not a config").unwrap();
        assert_eq!(find_configs_in_dir(cfg_dir).unwrap().count(), 10);

        let mut f = File::create(cfg_dir.join("fake.toml")).unwrap();
        f.write_all(b"[this]\nis = \"valid toml\"").unwrap();
        assert_eq!(find_configs_in_dir(cfg_dir).unwrap().count(), 10);

        fs::create_dir(cfg_dir.join("fake.dir")).unwrap();
        assert_eq!(find_configs_in_dir(cfg_dir).unwrap().count(), 10);

        write_n_configs(33, cfg_dir);
        assert_eq!(find_configs_in_dir(cfg_dir).unwrap().count(), 32);
    }

    #[test]
    fn test_do_not_promiscuously_load_any_toml() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("fake.toml");
        let mut f = File::create(&cfg_path).unwrap();
        f.write_all(b"[this]\nis = \"valid toml\"").unwrap();

        let rtd = Rtd::new()
            .conf(&cfg_path)
            .load();

        assert!(rtd.is_err());
    }

    #[test]
    fn test_allow_loading_with_missing_optional_fields() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("fake.toml");
        let mut f = File::create(&cfg_path).unwrap();
        f.write_all(b"[connection]\n[features]\n[parameters]\n").unwrap();

        Rtd::new()
            .conf(&cfg_path)
            .load()
            .unwrap();
    }
}
