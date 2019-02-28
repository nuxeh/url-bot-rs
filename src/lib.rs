extern crate irc;
extern crate rusqlite;
extern crate docopt;
#[macro_use]
extern crate serde_derive;
extern crate itertools;
extern crate regex;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate failure;
extern crate time;
extern crate reqwest;
extern crate cookie;
extern crate image;
extern crate serde_rusqlite;
extern crate mime;
extern crate humansize;
extern crate unicode_segmentation;
extern crate scraper;
extern crate toml;
extern crate directories;
#[macro_use]
extern crate log;
extern crate atty;
extern crate stderrlog;

pub mod sqlite;
pub mod http;
pub mod title;
pub mod config;
pub mod message;
pub mod buildinfo {
   include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

lazy_static! {
    pub static ref VERSION: String = format!(
        "v{}{} (build: {})",
        buildinfo::PKG_VERSION,
        buildinfo::GIT_VERSION.map_or_else(
            || String::from(""),
            |v| format!(" (git {})", v)),
        buildinfo::PROFILE
    );
}
