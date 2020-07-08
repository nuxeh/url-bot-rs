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

impl TitlePlugin for ImgurPlugin {
    fn name(&self) -> String {
        "imgur".into()
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

        let req_url = Url::parse("https://api.imgur.com/3/")?
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
