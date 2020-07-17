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
    use tiny_http::{Response, Header};

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
        assert_eq!(plugin.check(&config, &bad_url), false);

        // API key is set
        config.imgur.api_key = String::from("bar");
        assert_eq!(plugin.check(&config, &url), true);
        assert_eq!(plugin.check(&config, &bad_url), false);
    }

    #[test]
    fn evaluate() {
        let reponse_body = r#"{"data":{"id":"0pVuZq8","title":"Ducks and Dog","description":null,"datetime":1594707178,"cover":"0EDF1TX","cover_width":640,"cover_height":611,"account_url":"s3krit","account_id":123456789,"privacy":"hidden","layout":"blog","views":20291,"link":"https:\/\/imgur.com\/a\/0pVuZq8","ups":402,"downs":6,"points":396,"score":406,"is_album":true,"vote":null,"favorite":false,"nsfw":false,"section":"","comment_count":11,"favorite_count":65,"topic":"No Topic","topic_id":29,"images_count":1,"in_gallery":true,"is_ad":false,"tags":[{"name":"funny","display_name":"funny","followers":4476450,"total_items":2682845,"following":false,"is_whitelisted":false,"background_hash":"9r1qCDq","thumbnail_hash":null,"accent":"633875","background_is_animated":false,"thumbnail_is_animated":false,"is_promoted":false,"description":"LOLs, ROFLs, LMAOs","logo_hash":null,"logo_destination_url":null,"description_annotations":{}},{"name":"aww","display_name":"aww","followers":2413704,"total_items":751957,"following":false,"is_whitelisted":false,"background_hash":"avRBRpN","thumbnail_hash":null,"accent":"60AEBB","background_is_animated":false,"thumbnail_is_animated":false,"is_promoted":false,"description":"cute and adorable","logo_hash":null,"logo_destination_url":null,"description_annotations":{}},{"name":"dogs_are_the_best_people","display_name":"dogs are the best people","followers":61705,"total_items":26874,"following":false,"is_whitelisted":false,"background_hash":"DDUrxGU","thumbnail_hash":null,"accent":"93BDFF","background_is_animated":false,"thumbnail_is_animated":false,"is_promoted":false,"description":"","logo_hash":null,"logo_destination_url":null,"description_annotations":{}}],"ad_type":0,"ad_url":"","in_most_viral":true,"include_album_ads":false,"images":[{"id":"0EDF1TX","title":null,"description":null,"datetime":1594707145,"type":"image\/jpeg","animated":false,"width":640,"height":611,"size":120730,"views":11965,"bandwidth":1444534450,"vote":null,"favorite":false,"nsfw":null,"section":null,"account_url":null,"account_id":null,"is_ad":false,"in_most_viral":false,"has_sound":false,"tags":[],"ad_type":0,"ad_url":"","edited":"0","in_gallery":false,"link":"https:\/\/i.imgur.com\/0EDF1TX.jpg","comment_count":null,"favorite_count":null,"ups":null,"downs":null,"points":null,"score":null}],"ad_config":{"safeFlags":["in_gallery","sixth_mod_safe","gallery","album"],"highRiskFlags":[],"unsafeFlags":[],"wallUnsafeFlags":[],"showsAds":true}},"success":true,"status":200}"#;

        let bind = "127.0.0.1:28278";
        let url = format!("http://{}/r_rel", bind);
        let h_loc = Header::from_bytes("location", "/r_rel_r").unwrap();

        let server_thread = thread::spawn(move || {
            let server = tiny_http::Server::http(bind).unwrap();
            for _ in 0..2 {
                let rq = server.recv().unwrap();

                match rq.url() {
                    // redirection
                    "/r_rel" => {
                        let resp = Response::from_string("")
                            .with_status_code(301)
                            .with_header(h_loc.clone());
                        thread::sleep(Duration::from_millis(10));
                        rq.respond(resp).unwrap();
                    },
                    // response
                    "/r_rel_r" => {
                        let resp = Response::from_string("<title>hello</title>");
                        thread::sleep(Duration::from_millis(10));
                        rq.respond(resp).unwrap();
                    },
                    _ => (),
                }
            }
        });

        thread::sleep(Duration::from_millis(50));

        resolve_url(
            &url,
            &Rtd::new().init_http_client().unwrap()
        ).unwrap();

        server_thread.join().unwrap();

    }
}
