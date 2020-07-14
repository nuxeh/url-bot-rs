use reqwest::{Url, header, header::HeaderMap};
use failure::{Error, bail};
use serde::{Serialize, Deserialize};

use crate::{
    plugin_conf, config::Rtd,
    plugins::{TitlePlugin, PluginConfig},
};

/// Imgur title plugin configuration structure
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Config {
    api_key: String,
}

/// Imgur title plugin
pub struct ImgurPlugin {}

#[cfg(not(test))]
static REQUEST_URL: &str = "https://api.imgur.com/3/";

impl TitlePlugin for ImgurPlugin {
    fn name(&self) -> &'static str {
        "imgur"
    }

    fn check(&self, config: &PluginConfig, url: &Url) -> bool {
        if config.imgur.api_key.is_empty() {
            false
        } else {
            url.domain() == Some("imgur.com") && url.path().starts_with("/gallery/")
        }
    }

    fn evaluate(&self, rtd: &Rtd , url: &Url) -> Result<String, Error> {
        let mut headers = HeaderMap::new();

        let req_url = Url::parse(REQUEST_URL)?
            .join(&url.path()[1..])? // remove leading /
            .into_string();
        let header_content = format!("Client-ID {}", &plugin_conf!(rtd, imgur).api_key);

        headers.insert(header::AUTHORIZATION, header_content.parse()?);

        let client = match rtd.get_client() {
            Ok(c) => c,
            _ => bail!("Can't get http client"),
        };

        let res = client
            .request_with_headers(&req_url, headers)?
            .json::<Resp>()?;

        Ok(res.data.title)
    }
}

// Structures used for typed JSON parsing

#[derive(Debug, Deserialize)]
struct Resp {
    data: Data,
}

#[derive(Debug, Deserialize)]
struct Data {
    title: String,
}

// Tests

#[cfg(test)]
static REQUEST_URL: &str = "https://localhost:8266/test";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name() {
        let plugin = ImgurPlugin {};
        assert_eq!(plugin.name(), "imgur");
    }

    #[test]
    fn check() {
        let plugin = ImgurPlugin {};
        let mut config = PluginConfig::default();
        let url = Url::parse("https://imgur.com/gallery/foo").unwrap();
        let bad_url = Url::parse("https://i.imgur.com/foo").unwrap();

        // No API key set
        assert_eq!(plugin.check(&config, &url), false);
        // Bad URL
        assert_eq!(plugin.check(&config, &bad_url), false);

        // API key is set
        config.imgur.api_key = String::from("bar");
        assert_eq!(plugin.check(&config, &url), true);
        assert_eq!(plugin.check(&config, &bad_url), false);
    }

    #[test]
    fn evaluate() {

    }
}
