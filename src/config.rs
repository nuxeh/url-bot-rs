use std::fs;
use toml;
use std::path::{Path, PathBuf};
use irc::client::data::Config as IrcConfig;
use failure::Error;

#[derive(Debug, Deserialize)]
pub struct Conf {
    #[serde(skip)]
    pub file_path: PathBuf,

    #[serde(rename = "connection")]
    pub client: IrcConfig,
    pub features: Features,
}

#[derive(Debug, Deserialize)]
pub struct Features {
    pub report_metadata: bool,
    pub report_mime: bool,
    pub mask_highlights: bool,
    pub send_notice: bool,
    #[serde(default)]
    pub url_limit: UrlLimit,
}

#[derive(Deserialize, Debug)]
pub struct UrlLimit(pub u8);
impl Default for UrlLimit {
    fn default() -> Self {
        UrlLimit(10)
    }
}

impl Conf {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        // Load entries via serde
        let conf = fs::read_to_string(path.as_ref())?;
        let mut conf: Conf = toml::de::from_str(&conf)?;
        conf.file_path = path.as_ref().to_path_buf();
        Ok(conf)
    }
}
