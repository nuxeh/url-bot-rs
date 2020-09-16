use lazy_static::lazy_static;

pub mod sqlite;
pub mod http;
pub mod title;
pub mod config;
pub mod message;
pub mod tld;
pub mod plugins;
pub mod buildinfo {
   include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

lazy_static! {
    pub static ref VERSION: String = format!(
        "v{}{} (build: {})",
        buildinfo::PKG_VERSION,
        buildinfo::GIT_VERSION
            .map_or_else(|| String::from(""), |v| format!(" (git {})", v)),
        buildinfo::PROFILE
    );
}
