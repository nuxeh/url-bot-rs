/*
 * Application configuration
 *
 */
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    collections::BTreeMap,
};
use irc::client::data::Config as IrcConfig;
use failure::{Error, bail};
use directories::{BaseDirs, ProjectDirs};
use serde_derive::{Serialize, Deserialize};
use log::info;

use crate::{
    VERSION,
    plugins::PluginConfig,
    http::{Retriever, RetrieverBuilder},
};

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
    pub cross_channel_history: bool,
    pub invite: bool,
    pub autosave: bool,
    pub send_errors_to_poster: bool,
    pub reply_with_errors: bool,
    pub partial_urls: bool,
    pub nick_response: bool,
    pub reconnect: bool,
}

#[macro_export]
macro_rules! feat {
    ($rtd:expr, $name:ident) => {
        $rtd.conf.features.$name
    };
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
    pub status_channels: Vec<String>,
    pub nick_response_str: String,
    pub reconnect_timeout: u64,
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            url_limit: 10,
            status_channels: vec![],
            nick_response_str: "".to_string(),
            reconnect_timeout: 10,
        }
    }
}

#[macro_export]
macro_rules! param {
    ($rtd:expr, $name:ident) => {
        $rtd.conf.params.$name
    };
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Http {
    pub timeout_s: u64,
    pub max_redirections: u8,
    pub max_retries: u8,
    pub retry_delay_s: u64,
    pub accept_lang: String,
    pub user_agent: Option<String>,
}

impl Default for Http {
    fn default() -> Self {
        Self {
            timeout_s: 10,
            max_redirections: 10,
            max_retries: 3,
            retry_delay_s: 5,
            accept_lang: "en".to_string(),
            user_agent: None,
        }
    }
}

#[macro_export]
macro_rules! http {
    ($rtd:expr, $name:ident) => {
        $rtd.conf.http_params.$name
    };
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Conf {
    #[serde(default)]
    pub plugins: PluginConfig,
    #[serde(default)]
    pub network: Network,
    #[serde(default)]
    pub features: Features,
    #[serde(default, rename = "parameters")]
    pub params: Parameters,
    #[serde(default, rename = "http")]
    pub http_params: Http,
    #[serde(default)]
    pub database: Database,
    #[serde(rename = "connection")]
    pub client: IrcConfig,
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

impl Conf {
    /// load configuration TOML from a file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let conf = fs::read_to_string(path.as_ref())?;
        let mut conf: Conf = toml::de::from_str(&conf)?;
        // insert the path the config was loaded from
        conf.path = Some(path.as_ref().to_path_buf());
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
            plugins: PluginConfig::default(),
            network: Network::default(),
            features: Features::default(),
            params: Parameters::default(),
            http_params: Http::default(),
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
            },
            path: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ConfSet {
    #[serde(flatten)]
    pub configs: BTreeMap<String, Conf>,
}

impl ConfSet {
    pub fn new() -> Self {
        ConfSet { configs: BTreeMap::new() }
    }

    /// load configuration TOML from a file
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let conf_string = fs::read_to_string(path.as_ref())?;
        let mut conf_set: ConfSet = toml::de::from_str(&conf_string)?;

        // populate path field of all configs
        conf_set.configs
            .iter_mut()
            .for_each(|(_, c)| c.path = Some(path.as_ref().to_path_buf()));

        Ok(conf_set)
    }

    /// write configuration to a file
    pub fn write(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        let mut file = File::create(path)?;
        file.write_all(toml::ser::to_string(&self)?.as_bytes())?;
        Ok(())
    }
}

/// Run-time configuration data.
#[derive(Default, Clone)]
pub struct Rtd {
    /// paths
    pub paths: Paths,
    /// configuration file data
    pub conf: Conf,
    /// HTTP client
    client: Option<Retriever>,
}

#[derive(Default, Clone)]
pub struct Paths {
    pub db: Option<PathBuf>,
}

impl Rtd {
    pub fn new() -> Self {
        Rtd::default()
    }

    /// Set the configuration
    pub fn conf(mut self, c: Conf) -> Self {
        self.conf = c;
        self
    }

    pub fn db(mut self, path: Option<&PathBuf>) -> Self {
        self.paths.db = path.map(|p| expand_tilde(p));
        self
    }

    pub fn init_http_client(mut self) -> Result<Self, Error> {
        let conf = &self.conf.http_params;

        let mut builder = RetrieverBuilder::new()
            .retry(conf.max_retries.into(), conf.retry_delay_s)
            .timeout(conf.timeout_s)
            .accept_lang(&conf.accept_lang)
            .redirect_limit(conf.max_redirections.into());

        if let Some(ref user_agent) = conf.user_agent {
            builder = builder.user_agent(user_agent);
        };

        self.client = Some(builder.build()?);

        Ok(self)
    }

    pub fn get_client(&self) -> Result<&Retriever, Error> {
        let client = match self.client.as_ref() {
            None => bail!("HTTP client not initialised"),
            Some(c) => c,
        };

        Ok(client)
    }

    /// Load the configuration file and return an Rtd.
    pub fn load(mut self) -> Result<Self, Error> {
        // get a database path
        self.paths.db = self.get_db_path().map(|p| expand_tilde(&p));

        if let Some(dp) = &self.paths.db {
            ensure_parent_dir(dp)?;
        }

        // set url-bot-rs version number in the irc client configuration
        self.conf.client.version = Some(VERSION.to_string());

        Ok(self)
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
        let mut path = self.conf.database.path.as_ref()
            .filter(|p| !p.is_empty())
            .map(PathBuf::from);

        if self.paths.db.is_some() && path.is_none() {
            path = self.paths.db.clone();
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

pub fn ensure_parent_dir(file: &Path) -> Result<bool, Error> {
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
        .filter(|e| !e.is_dir() && (Conf::load(e).is_ok() || ConfSet::load(e).is_ok()))
        .take(32))
}

/// Take a vector of paths to either configurations, or configuration sets,
/// and return a vector of configurations
pub fn load_flattened_configs(paths: Vec<PathBuf>) -> Vec<Conf> {
    let mut configs: Vec<Conf> = paths.iter()
        .filter_map(|p| Conf::load(p).ok())
        .collect();

    let mut set_configs: Vec<Conf> = paths.into_iter()
        .filter_map(|p| ConfSet::load(p).ok())
        .flat_map(|s| s.configs.values().cloned().collect::<Vec<Conf>>())
        .collect();

    configs.append(&mut set_configs);
    configs
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::env;
    use std::iter;
    use std::panic;

    #[test]
    /// test that the example configuration file parses without error
    fn load_example_configs() {
        Conf::load(&PathBuf::from("example.config.toml")).unwrap();
        ConfSet::load(&PathBuf::from("example.multi.config.toml")).unwrap();
    }

    #[test]
    fn load_write_default() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("config.toml");

        let conf = Conf::default();
        conf.write(&cfg_path).unwrap();

        let example = fs::read_to_string("example.config.toml").unwrap();
        let written = fs::read_to_string(cfg_path).unwrap();

        example.lines()
            .zip(written.lines())
            .for_each(|(a, b)| assert_eq!(a, b));
    }

    fn get_test_confset() -> ConfSet {
        let mut confset = ConfSet::new();

        let mut conf = Conf::default();
        conf.network.name = String::from("foo");
        confset.configs.insert("foo".to_string(), conf.clone());
        conf.network.name = String::from("bar");
        confset.configs.insert("bar".to_string(), conf);

        confset
    }

    #[test]
    fn load_write_default_set() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("config.toml");

        let conf = get_test_confset();
        conf.write(&cfg_path).unwrap();

        let example = fs::read_to_string("example.multi.config.toml").unwrap();
        let written = fs::read_to_string(cfg_path).unwrap();

        example.lines()
            .zip(written.lines())
            .for_each(|(a, b)| assert_eq!(a, b));
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

        let conf = Conf::load(&cfg_path).unwrap();
        let rtd = Rtd::new()
            .conf(conf)
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

        let conf = Conf::load(&cfg_path).unwrap();
        let rtd = Rtd::new()
            .conf(conf)
            .db(Some(&db_path))
            .load()
            .unwrap();

        assert_eq!(rtd.paths.db, Some(db_path_cfg));
    }

    #[test]
    fn sqlite_path_empty_config_path_does_not_override_explicit() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("config.toml");
        let db_path = tmp_dir.path().join("cfg.test.db");

        let mut cfg = Conf::default();
        cfg.features.history = true;
        cfg.database.db_type = DbType::Sqlite;
        cfg.database.path = Some("".to_string());
        cfg.write(&cfg_path).unwrap();

        let conf = Conf::load(&cfg_path).unwrap();
        let rtd = Rtd::new()
            .conf(conf)
            .db(Some(&db_path))
            .load()
            .unwrap();

        assert_eq!(rtd.paths.db, Some(db_path));
    }

    #[test]
    fn sqlite_path_default() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("config.toml");

        let mut cfg = Conf::default();
        cfg.features.history = true;
        cfg.database.db_type = DbType::Sqlite;
        cfg.write(&cfg_path).unwrap();

        let conf = Conf::load(&cfg_path).unwrap();
        let rtd = Rtd::new()
            .conf(conf)
            .load()
            .unwrap();

        let dirs = ProjectDirs::from("org", "", "url-bot-rs").unwrap();
        let default = dirs.data_local_dir().join("history.default.db");
        println!("database path: {}", default.to_str().unwrap());
        assert_eq!(rtd.paths.db, Some(default));

        cfg.network.name = "test_net".to_string();
        cfg.write(&cfg_path).unwrap();

        let conf = Conf::load(&cfg_path).unwrap();
        let rtd = Rtd::new()
            .conf(conf)
            .load()
            .unwrap();

        let default = dirs.data_local_dir().join("history.test_net.db");
        println!("database path: {}", default.to_str().unwrap());
        assert_eq!(rtd.paths.db, Some(default.clone()));

        cfg.database.path = Some("".to_string());
        cfg.write(&cfg_path).unwrap();

        let conf = Conf::load(&cfg_path).unwrap();
        let rtd = Rtd::new()
            .conf(conf)
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

        let conf = Conf::load(&cfg_path).unwrap();
        let rtd = Rtd::new()
            .conf(conf)
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

    fn print_diff(example: &str, default: &str) {
        // print diff (on failure)
        println!("Configuration diff (- example, + default):");
        for diff in diff::lines(&example, &default) {
            match diff {
                diff::Result::Left(l) => println!("-{}", l),
                diff::Result::Both(l, _) => println!(" {}", l),
                diff::Result::Right(r) => println!("+{}", r)
            }
        }
    }

    #[test]
    /// test that the example configuration matches default values
    fn example_conf_data_matches_generated_default_values() {
        let example = fs::read_to_string("example.config.toml").unwrap();
        let default = toml::ser::to_string(&Conf::default()).unwrap();

        print_diff(&example, &default);

        default.lines()
            .zip(example.lines())
            .for_each(|(a, b)| assert_eq!(a, b));
    }

    #[test]
    /// test that the example configuration matches default values
    fn example_conf_data_matches_generated_expected_values() {
        // construct the example
        let confset = get_test_confset();

        let example = fs::read_to_string("example.multi.config.toml").unwrap();
        let default = toml::ser::to_string(&confset).unwrap();

        print_diff(&example, &default);

        default.lines()
            .zip(example.lines())
            .for_each(|(a, b)| assert_eq!(a, b));
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

        assert!(Conf::load(&cfg_path).is_err());
    }

    #[test]
    fn test_allow_loading_with_missing_optional_fields() {
        let tmp_dir = tempdir().unwrap();
        let cfg_path = tmp_dir.path().join("fake.toml");
        let mut f = File::create(&cfg_path).unwrap();
        f.write_all(b"[connection]\n[features]\n[parameters]\n").unwrap();

        Conf::load(&cfg_path).unwrap();
    }

    #[test]
    fn test_macros() {
        let mut rtd = Rtd::default();
        assert_eq!(10, param!(rtd, url_limit));
        assert_eq!(10, http!(rtd, max_redirections));
        assert!(!feat!(rtd, reconnect));

        rtd.conf.params.url_limit = 100;
        assert_eq!(100, param!(rtd, url_limit));

        rtd.conf.http_params.max_redirections = 100;
        assert_eq!(100, http!(rtd, max_redirections));

        rtd.conf.features.reconnect = true;
        assert!(feat!(rtd, reconnect));
    }

    #[test]
    fn test_load_flattened_configs() {
        let tmp_dir = tempdir().unwrap();
        let mut paths: Vec<PathBuf> = vec![];

        // make 10 normal configuration files
        for c in 0..10 {
            let path = tmp_dir.path().join(format!("conf_{}.toml", c));
            Conf::default().write(&path).unwrap();
            paths.push(path);
        }

        // make 10 configuration sets
        let set = get_test_confset();
        for c in 0..10 {
            let path = tmp_dir.path().join(format!("conf_multi_{}.toml", c));
            set.write(&path).unwrap();
            paths.push(path);
        }

        let res = load_flattened_configs(paths);
        assert_eq!(res.iter().count(), 30);
    }
}
