extern crate built;
extern crate man;

use std::fs::File;
use std::io::Write;
use man::prelude::*;

fn main() {
    built::write_built_file().expect("Failed to store build-time information");
    generate_manpage();
}

fn generate_manpage() {
    let page = Manual::new("url-bot-rs")
        .about("\
            Standalone IRC bot; for resolving URLs posted, retrieving, and \
            posting page titles to a configurable IRC server and channels")
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
                .help("Enable verbose mode. Add extra -v flags for increased \
                    verbosity, and printing of debugging information. For \
                    example, this can include information pertaining to \
                    website retrieval and title parsing, or printing of all \
                    received irc messages."),
        )
        .flag(
            Flag::new()
                .short("-t")
                .long("--timestamp")
                .help("Force timestamps in the output, even if they have been \
                    automatically disabled."),
        )
        .option(
            Opt::new("database")
                .short("-d")
                .long("--db")
                .help("Path to store a sqlite database. If not provided, \
                    a default path is used, based on the XDG specification."),
        )
        .option(
            Opt::new("configuration")
                .short("-c")
                .long("--conf")
                .help("Path to read configuration file from. If not provided, \
                    a default path is used, based on the XDG specification."),
        )
        .custom(
            Section::new("configuration")
                .paragraph("\
                    Most settings are read from the configuration file. This \
                    includes the details used to connect to an IRC server, \
                    features, and some runtime parameters. Running for the \
                    first time, a default-valued configuration will be \
                    generated in either the default XDG config path, or in the \
                    location specified with --conf. Once the bot has been run \
                    for the first time, and the default configuration \
                    has been generated, this should be edited and the bot \
                    restarted.")
        )
        .render();

    let mut file = File::create("url-bot-rs.1").unwrap();
    file.write_all(page.as_bytes()).unwrap();
}
