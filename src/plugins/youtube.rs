use reqwest::Url;
use failure::{Error, bail};
use serde::{Serialize, Deserialize};

use crate::{
    plugin_conf, config::Rtd,
    plugins::{TitlePlugin, PluginConfig},
};

/// YouTube title plugin configuration structure
#[derive(Debug, Serialize, Deserialize, Default, Clone, PartialEq)]
#[serde(default)]
pub struct Config {
    api_key: String,
}

/// YouTube title plugin
pub struct YouTubePlugin {}

#[cfg(not(test))]
static REQUEST_URL: &str = "https://www.googleapis.com/youtube/v3/videos?part=snippet";

impl TitlePlugin for YouTubePlugin {
    fn name(&self) -> &'static str { "youtube" }

    fn check(&self, config: &PluginConfig, url: &Url) -> bool {
        if config.youtube.api_key.is_empty() {
            false
        } else {
            url.domain() == Some("youtube.com")
            || url.domain() == Some("www.youtube.com")
            || url.domain() == Some("youtu.be")
            || url.domain() == Some("music.youtube.com")
        }
    }

    fn evaluate(&self, rtd: &Rtd , url: &Url) -> Result<String, Error> {
        let video_id = match url.domain() {
            Some("youtu.be") => url.path()[1..].to_string(),
            Some("www.youtube.com") | Some("youtube.com") | Some("music.youtube.com") => {
                url
                    .query_pairs()
                    .filter(|(k, _)| k == "v")
                    .map(|(_, v)| v)
                    .collect()
            },
            _ => bail!("Unknown domain"),
        };

        let mut req_url = Url::parse(REQUEST_URL)?;
        req_url
            .query_pairs_mut()
            .append_pair("id", &video_id)
            .append_pair("key", &plugin_conf!(rtd, youtube).api_key);

        let client = match rtd.get_client() {
            Ok(c) => c,
            _ => bail!("Can't get http client"),
        };

        let mut res = client
            .request(&req_url.into_string())?
            .json::<Resp>()?;

        let first_item = match res.items.pop() {
            Some(v) => v,
            None => bail!("No list items in response"),
        };

        Ok(first_item.snippet.title)
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

// Tests

#[cfg(test)]
static REQUEST_URL: &str = "http://127.0.0.1:28285/v3/";

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        thread,
        time::Duration,
    };
    use tiny_http::Response;

    #[test]
    fn name() {
        let plugin = YouTubePlugin {};
        assert_eq!(plugin.name(), "youtube");
    }

    #[test]
    fn check() {
        let plugin = YouTubePlugin {};
        let mut config = PluginConfig::default();
        let url = Url::parse("https://www.youtube.com/watch?v=abc123def78").unwrap();
        let url2 = Url::parse("https://youtu.be/abc123def78").unwrap();
        let url3 = Url::parse("https://music.youtube.com/watch?v=abc123def78&feature=share").unwrap();
        let bad_url = Url::parse("https://google.com/").unwrap();

        // No API key set
        assert_eq!(plugin.check(&config, &url), false);
        assert_eq!(plugin.check(&config, &url2), false);
        assert_eq!(plugin.check(&config, &url3), false);
        assert_eq!(plugin.check(&config, &bad_url), false);

        // API key is set
        config.youtube.api_key = String::from("bar");
        assert_eq!(plugin.check(&config, &url), true);
        assert_eq!(plugin.check(&config, &url2), true);
        assert_eq!(plugin.check(&config, &url3), true);
        assert_eq!(plugin.check(&config, &bad_url), false);
    }

    #[test]
    fn evaluate_no_client() {
        let plugin = YouTubePlugin {};
        let rtd = Rtd::new();
        let url = "https://www.youtube.com/watch?v=abc123def78";
        let res = plugin.evaluate(&rtd, &url.parse().unwrap());
        assert!(res.is_err());
        if let Err(e) = res { assert_eq!(&format!("{}", e), "Can't get http client"); }
    }

    #[test]
    fn evaluate_bad_domain() {
        let plugin = YouTubePlugin {};
        let rtd = Rtd::new().init_http_client().unwrap();
        let url = "https://www.notyoutube.com/watch?v=abc123def78";
        let res = plugin.evaluate(&rtd, &url.parse().unwrap());
        assert!(res.is_err());
        if let Err(e) = res { assert_eq!(&format!("{}", e), "Unknown domain"); }
    }

    #[test]
    fn evaluate() {
        let plugin = YouTubePlugin {};
        let rtd = Rtd::new().init_http_client().unwrap();
        let bind = "127.0.0.1:28285";
        let url = "https://www.youtube.com/watch?v=abc123def78";
        let response_no_list_items = r#"{"kind":"youtube#videoListResponse","etag":"123456","items":[],"pageInfo":{"totalResults":1,"resultsPerPage":1}}"#;
        let response = r#"{"kind":"youtube#videoListResponse","etag":"123456","items":[{"kind":"youtube#video","etag":"123456","id":"abc123def78","snippet":{"publishedAt":"2020-08-10T11:45:00Z","channelId":"123456789abcdefg","title":"Glorious YouTube video","description":"","thumbnails":{"default":{"url":"","width":120,"height":90},"medium":{"url":"","width":320,"height":180},"high":{"url":"","width":480,"height":360},"standard":{"url":"","width":640,"height":480},"maxres":{"url":"","width":1280,"height":720}},"channelTitle":"A channel name","tags":[],"categoryId":"10","liveBroadcastContent":"none","localized":{"title":"Glorious YouTube video","description":""}}}],"pageInfo":{"totalResults":1,"resultsPerPage":1}}"#;

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http(bind).unwrap();
            for i in 0..2 {
            let rq = server.recv().unwrap();
                if rq.url().to_string().starts_with("/v3/") {
                    let resp = if i == 0 {
                        Response::from_string(response)
                    } else {
                        Response::from_string(response_no_list_items)
                    };
                    thread::sleep(Duration::from_millis(10));
                    rq.respond(resp).unwrap();
                }
            }
        });

        thread::sleep(Duration::from_millis(1000));

        let res = plugin.evaluate(&rtd, &url.parse().unwrap()).unwrap();
        assert_eq!(res, String::from("Glorious YouTube video"));

        thread::sleep(Duration::from_millis(1000));

        let res = plugin.evaluate(&rtd, &url.parse().unwrap());
        assert!(res.is_err());
        if let Err(e) = res { assert_eq!(&format!("{}", e), "No list items in response"); }

        server_thread.join().unwrap();
    }
}
