use reqwest::Url;
use failure::Error;
use serde::{Serialize, Deserialize};

use crate::config::Rtd;

pub trait TitlePlugin {
    /// Get the name of the plugin
    fn name(&self) -> String;
    /// Check to see if the token is a viable candidate for running the plugin
    fn check(&self, config: &PluginConfig, url: &Url) -> bool;
    /// Run the plugin to get a title
    fn evaluate(&self, rtd: &Rtd, url: &Url) -> Result<String, Error>;
}

/// Plugin includes
pub mod imgur;

/// Plugin configuration structures
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct PluginConfig {
    imgur: imgur::Config,
}

/// Plugin instantiations (as trait objects)
pub const TITLE_PLUGINS: [&dyn TitlePlugin; 1] = [
    &imgur::ImgurPlugin {},
];

#[macro_export]
macro_rules! plugin_conf {
    ($rtd:expr, $name:ident) => {
        $rtd.conf.plugins.$name
    };
}
