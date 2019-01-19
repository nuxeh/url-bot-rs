[![build status](https://api.travis-ci.org/nuxeh/url-bot-rs.png?branch=master)](https://travis-ci.org/nuxeh/url-bot-rs)

# url-bot-rs

URL title fetching bot for IRC in Rust. The bot monitors all messages sent to
it in any IRC channels it's joined to, if any messages contain URLs, the bot
fetches the page and extracts the title, posting the result on the same
channel, adding a certain je ne sais quoi to your IRC experience.

For example:

    <user> http://rust-lang.org/
    <url-bot-rs> ⤷ The Rust Programming Language

## Build

### Get rust

[https://www.rust-lang.org/en-US/install.html](https://www.rust-lang.org/en-US/install.html)

### Build

    git clone https://github.com/nuxeh/url-bot-rs
    cd url-bot-rs
    cargo build

### Run tests

    cargo test

## Configuration file

A configuration file is required to specify IRC server details and other
general settings for the bot, a path to this config can be specified manually
with the `--conf=<path>` command line option. If not provided, url bot will
look in a default path for your platform, e.g. on Linux it will use the XDG
specification:

    ~/.config/url-bot-rs/config.toml

or, if `$XDG_CONFIG_PATH` is set:

    $XDG_CONFIG_HOME/url-bot-rs/config.toml

### Configuration file options

The configuration includes settings pertaining to the IRC server the bot will
connect to, including among other things:

- Address of the IRC server
- Connection credentials
- The nick the bot will use when joining
- Channels to join

It is also possible to configure a number of optional features for the bot's
operation, specified in the `[features]` section:

- `mask_highlights` (bool) inserts invisible characters to defeat highlight
  regexes
- `send_notice` (bool) causes the bot to respond with notices rather than
  private messages
- `report_metadata` (bool) if enabled, causes image metadata to be reported
- `report_mime` (bool) if enabled, causes mime types to be reported, if no
  other title or metadata is found.
- `history` (bool) enable previous post information using a database

The `[parameters]` section includes a number of tunable parameters:

- `url_limit` (u8) max number of URLs to process for each message (default: 10)
- `user_agent` (String) the user agent to use for http content requests
- `accept_lang` (String) language requested in http content requests
  (default: "en")

The `[database]` section contains options for the database, as follows:

- `path` (String) is the path to a database file (for `sqlite`)
- `type` (String) is the type of database to use, e.g. `sqlite`

If no configuration file exists at the expected location, a default-valued
configuration file will be created. An example configuration is provided as
`example.config.toml` in this repository.

## Database

A database may be specified, which is used to cache posted links, so that if
the same URL is posted again, the original poster and the time posted is added
to the returned message. This feature can be enabled or disabled within the
`[features]` section of the configuration file.

History is also enabled if a path is specified with `--db=<path>` or in the
configuration, the given path will be used to store a SQLite database,
otherwise a default path will be used according to the XDG specification. If no
file exists at the specified path, it will be created.

If history is enabled, no database type is specified in the `[database]`
section of the configuration, and no database path has been specified, an
in-memory database will be used.

## Install from source

### Cargo

    cargo install --git https://github.com/nuxeh/url-bot-rs

### Debian/Ubuntu (linux)

    git clone https://github.com/nuxeh/url-bot-rs
    cd url-bot-rs
    cargo install cargo-deb
    cargo deb --install

After this, the bot may be started manually by running `url-bot-rs`.

## Running as a service

### Debian package install

If you install using the Debian package, this also installs a systemd unit for
running as a service, e.g.:

    systemctl enable url-bot-rs.service
    systemctl start url-bot-rs.service
    systemctl status url-bot-rs.service

...etc.

## Additional command line options

- Usage is printed by providing `--help` on run.
- To print some additional runtime information, add `-v` or `--verbose`.
- To print all received IRC messages, along with HTTP response data to the
  console, add `-D` or `--debug`.

## IRC

There is an IRC channel on [Moznet](https://wiki.mozilla.org/IRC), `#url-bot-rs`.
