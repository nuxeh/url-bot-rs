extern crate built;
extern crate man;

use std::env;
use std::fs::{File, create_dir};
use std::io::Write;
use std::path::PathBuf;
use man::prelude::*;

fn main() {
    println!("cargo:rerun-if-changed=src");
    built::write_built_file().expect("Failed to store build-time information");
    generate_manpage();
}

fn get_assets_dir() -> PathBuf{
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = PathBuf::from(&out_dir);

    let mut path = out_path
      .ancestors()
      .nth(4)
      .unwrap()
      .to_owned();
    path.push("assets");

    if !path.exists() {
        create_dir(&path).expect("could not create assets dir");
    }

    path
}

fn generate_manpage() {
    let page = Manual::new("url-bot-rs")
        .about("\
            Standalone IRC bot; for resolving URLs posted, retrieving, and \
            posting page titles to a configurable IRC servers and channels")
        .author(Author::new("Ed Cragg").email("drq.11325@gmail.com"))
        .flag(
            Flag::new()
                .long("--version")
                .help("Print version information."),
        )
        .flag(
            Flag::new()
                .long("--help")
                .help("Show usage."),
        )
        .flag(
            Flag::new()
                .short("-v")
                .long("--verbose")
                .help("\
                    Enable verbose mode. May be specified multiple times for \
                    more verbosity."),
        )
        .option(
            Opt::new("configuration")
                .short("-c")
                .long("--conf")
                .help("\
                    Path to read a single configuration file from. May be \
                    specified multiple times."),
        )
        .option(
            Opt::new("configuration directory")
                .short("-d")
                .long("--conf-dir")
                .help("\
                      Directory containing configurations. The path will be \
                      searched for valid configuration files, each \
                      configuration may contain connection information for a \
                      separate IRC network, with each configuration found \
                      starting a url-bot-rs instance on a thread. Any \
                      configuration files for which the network.enable field \
                      is false will be ignored. May be specified multiple \
                      times."),
        )
        .flag(
            Flag::new()
                .short("-t")
                .long("--timestamp")
                .help("\
                    Force timestamps to be printed, even when they would \
                    otherwise be disabled, e.g. when output is piped."),
        )
        .custom(
            Section::new("configuration")
                .paragraph("\
                    Most settings are read from the configuration file. This \
                    includes the details used to connect to an IRC server, \
                    features, and some runtime parameters. Running for the \
                    first time, a default-valued configuration will be \
                    generated in either the default XDG config path, or in the \
                    location specified with --conf.")
        )
        .render();

    let assets_dir = get_assets_dir();
    let dest_path = assets_dir.join("url-bot-rs.1");
    let mut file = File::create(&dest_path).unwrap();
    file.write_all(page.as_bytes()).unwrap();
}
