use reqwest::{Url, header, header::HeaderMap};
use failure::{Error, bail};
use serde::{Serialize, Deserialize};

use crate::{
    plugin_conf, config::Rtd,
    plugins::{TitlePlugin, PluginConfig},
};

/// Vimeo title plugin configuration structure
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct Config {
    api_key: String
}

/// Vimeo title plugin
pub struct VimeoPlugin {}

#[cfg(not(test))]
static REQUEST_URL: &str = "https://api.vimeo.com/";

impl TitlePlugin for VimeoPlugin {

    fn name(&self) -> &'static str { "vimeo" }

    fn check(&self, config: &PluginConfig, url:&Url) -> bool {
        if config.vimeo.api_key.is_empty() {
            false
        } else {
            url.domain() == Some("vimeo.com")
            || url.domain() == Some("www.vimeo.com")
        }
    }

    fn evaluate(&self, rtd: &Rtd, url: &Url) -> Result<String, Error> {
        let video_id = url.path()[1..].to_string();
        let mut req_url = Url::parse(REQUEST_URL)?;
        req_url.path_segments_mut().unwrap()
            .push("videos")
            .push(&video_id);
        let client = match rtd.get_client() {
            Ok(c) => c,
            _ => bail!("Can't get http client"),
        };

        let header_content = format!("bearer {}", &plugin_conf!(rtd, vimeo).api_key);
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, header_content.parse()?);

        let res = client
            .request_with_headers(&req_url.into_string(), headers)?
            .json::<Resp>()?;

        Ok(res.name)
    }
}

// structures for JSON parsing

#[derive(Debug, Deserialize)]
struct Resp {
    name: String
}

#[cfg(test)]
static REQUEST_URL: &str = "http://127.0.0.1:28286/";

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
        let plugin = VimeoPlugin {};
        assert_eq!(plugin.name(), "vimeo");
    }

    #[test]
    fn check() {
        let plugin = VimeoPlugin {};
        let mut config = PluginConfig::default();
        let url1 = Url::parse("https://vimeo.com/53603603").unwrap();
        let url2 = Url::parse("https://www.vimeo.com/53603603").unwrap();
        let url3 = Url::parse("https://www.vimeo.com/53603603#t=20s").unwrap();
        let bad_url = Url::parse("https://www.wimeo.com").unwrap();

        assert_eq!(plugin.check(&config, &url1), false);
        assert_eq!(plugin.check(&config, &url2), false);
        assert_eq!(plugin.check(&config, &url3), false);
        assert_eq!(plugin.check(&config, &bad_url), false);

        config.vimeo.api_key = String::from("baz");
        assert_eq!(plugin.check(&config, &url1), true);
        assert_eq!(plugin.check(&config, &url2), true);
        assert_eq!(plugin.check(&config, &url3), true);
        assert_eq!(plugin.check(&config, &bad_url), false);
    }

    #[test]
    fn evaluate_no_client() {
        let plugin = VimeoPlugin {};
        let rtd = Rtd::new();
        let url = "https://vimeo.com/53603603";
        let res = plugin.evaluate(&rtd, &url.parse().unwrap());
        assert!(res.is_err());
        if let Err(e) = res { assert_eq!(&format!("{}", e), "Can't get http client"); }
    }

    #[test]
    fn evaluate() {
        let plugin = VimeoPlugin {};
        let rtd = Rtd::new().init_http_client().unwrap();
        let bind = "127.0.0.1:28286";
        let url = "https://vimeo.com/53603603";
        let response=r#"{"uri":"/videos/53603603","name":"CAPTAIN MURPHY'S DUALITY","description":"HTTP://CAPTAINMURPHY.XXX\nVideobyXavierMagotakaRevenge","type":"video","link":"https://vimeo.com/53603603","duration":2130,"width":450,"language":null,"height":360,"embed":{"html":"<iframesrc=\"https://player.vimeo.com/video/53603603?badge=0&amp;autopause=0&amp;player_id=0&amp;app_id=219170\"width=\"450\"height=\"360\"frameborder=\"0\"allow=\"autoplay;fullscreen;picture-in-picture\"allowfullscreentitle=\"CAPTAINMURPHY&amp;#039;SDUALITY\"></iframe>","badges":{"hdr":false,"live":{"streaming":false,"archived":false},"staff_pick":{"normal":false,"best_of_the_month":false,"best_of_the_year":false,"premiere":false},"vod":false,"weekend_challenge":false}},"created_time":"2012-11-15T15:47:01+00:00","modified_time":"2021-07-10T14:22:06+00:00","release_time":"2012-11-15T15:47:01+00:00","content_rating":["unrated"],"license":null,"privacy":{"view":"anybody","embed":"public","download":false,"add":false,"comments":"nobody"}}"#;

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http(bind).unwrap();
            let rq = server.recv().unwrap();
            if rq.url().to_string().starts_with('/') {
                    let resp = Response::from_string(response);
                    thread::sleep(Duration::from_millis(10));
                    rq.respond(resp).unwrap();
            }
        });

        thread::sleep(Duration::from_millis(1000));

        let res = plugin.evaluate(&rtd, &url.parse().unwrap()).unwrap();
        assert_eq!(res, String::from("CAPTAIN MURPHY'S DUALITY"));

        server_thread.join().unwrap();
    }
}
