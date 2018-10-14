use std::process;
use std::fs::File;
use std::io::Read;
use toml::from_str;

#[derive(Debug, Deserialize)]
struct Conf {
    features: Option<ConfOpts>,
}

#[derive(Debug, Deserialize, Default)]
pub struct ConfOpts {
    pub report_metadata: Option<bool>,
    pub report_mime: Option<bool>,
    pub mask_highlights: Option<bool>,
    pub send_notice: Option<bool>,
}

pub fn load(conf_file: &str) -> ConfOpts {
    let mut conf_string = String::new();

    File::open(conf_file).and_then(|mut f| {
        f.read_to_string(&mut conf_string)
    }).unwrap_or_else(|err| {
        eprintln!("Error loading configuration: {}", err);
        process::exit(1);
    });

    let conf: Conf = from_str(&conf_string).unwrap_or_else(|err| {
        eprintln!("Error parsing configuration: {}", err);
        process::exit(1);
    });

    conf.features.unwrap_or(ConfOpts::default())
}
