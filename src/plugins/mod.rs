use reqwest::Url;
use failure::Error;
use serde::{Serialize, Deserialize};

use crate::config::Rtd;

pub trait TitlePlugin {
    /// Get the name of the plugin
    fn name(&self) -> &'static str;
    /// Check to see if the token is a viable candidate for running the plugin
    fn check(&self, config: &PluginConfig, url: &Url) -> bool;
    /// Run the plugin to get a title
    fn evaluate(&self, rtd: &Rtd, url: &Url) -> Result<String, Error>;
}

/// Plugin includes
pub mod imgur;
pub mod youtube;
pub mod vimeo;

/// Plugin configuration structures
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct PluginConfig {
    imgur: imgur::Config,
    youtube: youtube::Config,
    vimeo: vimeo::Config,
}

/// Plugin instantiations (as trait objects)
pub const TITLE_PLUGINS: [&dyn TitlePlugin; 3] = [
    &imgur::ImgurPlugin {},
    &youtube::YouTubePlugin {},
    &vimeo::VimeoPlugin {},
];

#[macro_export]
macro_rules! plugin_conf {
    ($rtd:expr, $name:ident) => {
        $rtd.conf.plugins.$name
    };
}
