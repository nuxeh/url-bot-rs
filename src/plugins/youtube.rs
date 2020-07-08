use reqwest::Url;
use failure::{Error, bail};
use serde::{Serialize, Deserialize};

use crate::{
    plugin_conf, config::Rtd,
    plugins::{TitlePlugin, PluginConfig},
};

/// YouTube title plugin configuration structure
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Config {
    api_key: String,
}

/// YouTube title plugin
pub struct YouTubePlugin {}

impl TitlePlugin for YouTubePlugin {
    fn name(&self) -> String {
        "youtube".into()
    }

    fn check(&self, config: &PluginConfig, url: &Url) -> bool {
        if config.youtube.api_key.is_empty() {
            false
        } else {
            url.domain() == Some("youtube.com")
            || url.domain() == Some("youtu.be")
        }
    }

    fn evaluate(&self, rtd: &Rtd , url: &Url) -> Result<String, Error> {
        let video_id = match url.domain() {
            Some("youtu.be") => url.path()[1..].to_string(),
            Some("www.youtube.com") | Some("youtube.com") => {
                url
                    .query_pairs()
                    .filter(|(k, _)| k == "v")
                    .map(|(_, v)| v)
                    .collect()
            },
            _ => bail!("Unknown domain"),
        };

        let mut req_url = Url::parse("https://www.googleapis.com/youtube/v3/videos?part=snippet")?;
        req_url
            .query_pairs_mut()
            .append_pair("id", &video_id)
            .append_pair("key", &plugin_conf!(rtd, youtube).api_key);

        let client = match rtd.get_client() {
            Ok(c) => c,
            _ => bail!("Can't get http client"),
        };

        let res = client
            .request(&req_url.into_string())?
            .json::<Resp>()?;

        let first_item = match res.items.get(0) {
            Some(v) => v,
            None => bail!("No list items in response"),
        };

        Ok(first_item.snippet.title.clone())
    }
}

// Structures used for typed JSON parsing

#[derive(Debug, Deserialize)]
struct Resp {
    items: Vec<Item>,
}

#[derive(Debug, Deserialize)]
struct Item {
    snippet: Snippet,
}

#[derive(Debug, Deserialize)]
struct Snippet {
    title: String,
}
