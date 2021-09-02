![url-bot-rs](./logo.svg "url-bot-rs")

[![build](https://github.com/nuxeh/url-bot-rs/workflows/build/badge.svg)](https://github.com/nuxeh/url-bot-rs/actions?query=branch%3Amaster+event%3Apush+workflow%3Abuild)
[![test](https://github.com/nuxeh/url-bot-rs/workflows/tests/badge.svg)](https://github.com/nuxeh/url-bot-rs/actions?query=branch%3Amaster+event%3Apush+workflow%3Atests)
[![clippy](https://github.com/nuxeh/url-bot-rs/workflows/clippy/badge.svg)](https://github.com/nuxeh/url-bot-rs/actions?query=branch%3Amaster+event%3Apush+workflow%3Aclippy)
[![macOS](https://github.com/nuxeh/url-bot-rs/workflows/macOS/badge.svg)](https://github.com/nuxeh/url-bot-rs/actions?query=branch%3Amaster+event%3Apush+workflow%3AmacOS)
[![windows](https://github.com/nuxeh/url-bot-rs/workflows/windows/badge.svg)](https://github.com/nuxeh/url-bot-rs/actions?query=branch%3Amaster+event%3Apush+workflow%3Awindows)
[![coveralls](https://img.shields.io/coveralls/github/nuxeh/url-bot-rs/master)](https://coveralls.io/github/nuxeh/url-bot-rs?branch=master)
[![codecov](https://codecov.io/gh/nuxeh/url-bot-rs/branch/master/graph/badge.svg)](https://codecov.io/gh/nuxeh/url-bot-rs)
[![crates.io](https://img.shields.io/crates/v/url-bot-rs)](https://crates.io/crates/url-bot-rs)

URL title fetching bot for IRC in Rust. The bot monitors all messages sent to
it in any IRC channels it's joined to, if any messages contain URLs, the bot
fetches the page and extracts the title, posting the result on the same
channel, adding a certain je ne sais quoi to your IRC experience.

For example:

    <user> http://rust-lang.org/
    <url-bot-rs> ⤷ The Rust Programming Language

## Dependencies

Currently, SQLite is a required dependency.

On Linux or OSX, you can install SQLite as follows:

| Platform | Installation command                               |
|----------|----------------------------------------------------|
| Debian   | `apt install libsqlite3-dev`                       |
| OSX      | `brew install sqlite3`                             |

Ubuntu also needs additional dependencies for TLS.
These can be satisfied with:

| Platform | Installation command                               |
|----------|----------------------------------------------------|
| Ubuntu   | `apt install libssl-dev pkg-config`                       |

On Windows, or if preferred, also for Linux or OSX, bundled SQLite can also be
used, where it is built and linked statically without external dependencies, by
specifying the `sqlite_bundled` feature when building with Cargo, e.g.:

    cargo build --features "sqlite_bundled"
    cargo install url-bot-rs --features "sqlite_bundled"

When using bundled SQLite, no external dependencies are required.

## Quick install

    cargo install url-bot-rs

Or, with bundled SQLite:

    cargo install url-bot-rs --features "sqlite_bundled"

To get started quickly with a working configuration, run `url-bot-rs` with no
parameters, and edit the file as shown below.

| Platform | Configuration path                                              |
|----------|-----------------------------------------------------------------|
| Linux    | `~/.config/url-bot-rs/config.toml`                              |
| OSX      | `~/Library/Preferences/org.url-bot-rs/config.toml`              |
| Windows  | `C:\Users\<user>\AppData\Roaming\url-bot-rs\config\config.toml` |

## Packages

Debian packages for releases are available [on
Github](https://github.com/nuxeh/url-bot-rs/releases).

Build and install local snap packages like so:

```shell
git clone https://github.com/nuxeh/url-bot-rs.git
sudo snap install snapcraft
sudo snap install url-bot-rs*.snap --dangerous
```

## Build

### Get Rust

[https://www.rust-lang.org/en-US/install.html](https://www.rust-lang.org/en-US/install.html)

### Build

    git clone https://github.com/nuxeh/url-bot-rs
    cd url-bot-rs
    cargo build

### Run tests

    cargo test

## Configuration

A configuration file is required to specify IRC server details, features to
enable, database setting, and other general settings for the bot; a path to
this config can be specified manually with the `--conf=<path>` command line
option.

If not provided, url bot will look in a default path for your platform, e.g. on
Linux the XDG specification will be used.

First, the following directory will be searched for valid configurations:

    ~/.config/url-bot-rs/

or, if `$XDG_CONFIG_PATH` is set:

    $XDG_CONFIG_HOME/url-bot-rs/

If no configurations are found in this directory, a default-valued
configuration will be created at:

    ~/.config/url-bot-rs/config.toml

or, if `$XDG_CONFIG_PATH` is set:

    $XDG_CONFIG_HOME/url-bot-rs/config.toml

The `--conf` parameter may be provided multiple times, in order to connect to
multiple servers/networks.

Additionally, an additional search path may be specified by providing the
`--conf-dir=<dir>` CLI argument, with the effect that any valid configurations
existing non-recursively under this path will be loaded. This option may also
be specified multiple times.

When searching for configurations using the `--conf-dir` option, any
configurations in which `network.enable` is false will not be loaded.

### Configuration file options

The configuration includes settings pertaining to the IRC server the bot will
connect to, including among other things:

- Address of the IRC server
- Connection credentials
- The nick the bot will use when joining
- Channels to join

The `[network]` section gives some metadata for the network the configuration
will connect to, including:

- `name` (string) an identifier for the network.
- `enable` (bool) whether to enable this network - only used when a
  configuration is found in a search path using the `--conf-dir` CLI argument,
  and ignored if the configuration is explicitly loaded.

It is also possible to configure a number of optional features for the bot's
operation, specified in the `[features]` section:

- `mask_highlights` (bool) inserts invisible characters to defeat highlight
  regexes.
- `send_notice` (bool) causes the bot to respond with notices rather than
  private messages.
- `report_metadata` (bool) if enabled, causes image metadata to be reported.
- `report_mime` (bool) if enabled, causes mime types to be reported, if no
  other title or metadata is found.
- `history` (bool) enable previous post information using a database.
- `cross_channel_history` (bool) if enabled, only post pre-post information to
  the same channel as the original post.
- `invite` (bool) if enabled, `/invite` will cause the bot to join a channel.
- `autosave` (bool) if enabled, `/invite` and `/kick` will automatically write
  out the active configuration with an updated list of channels.
- `send_errors_to_poster` (bool) if enabled, sends any errors occurring when
  trying to resolve a link to the user posting the link, in a private message.
- `reply_with_errors` (bool) if enabled, always reply with error messages.
- `partial_urls` (bool) attempt to resolve titles for URLs without scheme, e.g.
  "docs.rs".
- `nick_response` (bool) respond with a message if bot is pinged in a message
  with no other action to perform.
- `reconnect` (bool) reconnect to the server after errors.

The `[parameters]` section includes a number of tunable parameters:

- `url_limit` (u8) max number of URLs to process for each message (default: 10).
- `status_channels` (list) channel(s) to create, join and message with any
  error messages produced from URL title retrieval.
- `nick_response_str` (string) the message to send for the nick response feature.
- `reconnect_timeout` (u32) amount of time to wait before reconnecting.

The `[http]` section contains options for HTTP requests used to obtain titles:

- `timeout_s` (u64) the timeout for any given request.
- `max_redirections` (u8) the maximum number of HTTP redirections to follow.
- `max_retries` (u8) the maximum number of times to retry after receiving an
  HTTP error response which may be temporary (e.g. `503 Service Unavailable`).
- `retry_delay_s` (u64) the number of seconds to wait before retrying.
- `accept_lang` (string) the `Accept-Lang` HTTP request header to send when
  making a request (default: "en").
- `user_agent` (string) the user agent string to send in HTTP requests.

The `[database]` section contains options for the database, as follows:

- `type` (string) is the type of database to use, e.g. `sqlite`.
- `path` (string) is the path to a database file (for `sqlite`).

If no configuration file exists at the location specified with the `--conf`
command line option, a default-valued configuration file will be created.

An example configuration is provided as `example.config.toml` in this repository.

## Plugins

A plugin system currently caters for using a number of JSON APIs to get better
title results. These each require additional configuration to be added to your
configuration file(s).

- Imgur

  ```
  [plugins.imgur]
  api_key = "your API key"
  ```

- Youtube

  ```
  [plugins.youtube]
  api_key = "your API key"
  ```

- Vimeo

  ```
  [plugins.vimeo]
  api_key = "your API key"
  ```

In each case, once a valid API key is present in the configuration, the plugin
will work automatically. For debugging, it may be helpful to use the
`url-bot-get` tool, using the `--plugin` option.

## Database

A database may be specified, which is used to cache posted links, so that if
the same URL is posted again, the original poster and the time posted is added
to the returned message. This feature can be enabled using the `history`
feature within the `[features]` section of the configuration file.

Currently supported database type strings are:

- `in-memory`
- `sqlite`

The database type may be specified in the `[database]` configuration section,
as field `type`. A corresponding path to use for the database may be given as
field, `path`.

For SQLite, if no path is specified, a default path will be used, and a
database will be created according to the network name specified in the
`[network]` section of the configuration.

## Install from source

### Cargo

    cargo install url-bot-rs

### Debian/Ubuntu (Linux)

    git clone https://github.com/nuxeh/url-bot-rs
    cd url-bot-rs
    cargo install cargo-deb
    cargo deb --install

### Nix

The following should be run on NixOS, or inside a Nix environment on another
OS.

    git clone https://github.com/nuxeh/url-bot-rs
    cd url-bot-rs
    nix-shell
    cargo build

## Running as a service

The bot can be run automatically as a service by `systemd`. This is set up
automatically in the case of a Debian package install, or alternatively can be
set up manually.

### Debian package install

If you install using the Debian package, a `url-bot-rs` user is created
automatically. Additionally, the systemd unit is installed, and the service is
enabled and started automatically, after installation.

The configuration should be customised as described in "Customising
configuration" below.

When uninstalling the Debian package, the `url-bot-rs` user nor its home
directory files are deleted, according to Debian packaging guidelines. This
keeps UIDs more deterministic, and allows re-installation or upgrade without
losing the bot's configuration.

### Manual systemd install

To set up systemd manually, the unit file must be copied, and the `url-bot-rs`
user must be created on the system. From inside the project repository:

    sudo install -m 644 systemd/url-bot-rs.service /etc/systemd/system/
    sudo useradd -m --system url-bot-rs --shell /usr/sbin/nologin
    sudo systemctl enable --now url-bot-rs.service

The configuration should be customised as described in "Customising
configuration" below.

### Customising configuration

Once started once, a default configuration is created in
`/home/url-bot-rs/.config/url-bot-rs/config.toml`, which should be edited, and
the bot restarted:

    sudo systemctl restart url-bot-rs.service

You can also place any configurations you wish to run under the default search
path:

    /home/url-bot-rs/.config/url-bot-rs/

### Checking status

To check status or get logs:

    systemctl status url-bot-rs.service
    sudo journalctl -u url-bot-rs.service

## `url-bot-rs` additional command line options

- Usage is printed by providing `--help` on run.
- To print additional runtime information, add `-v` or `--verbose`. The level
  of verbosity can be increased by adding extra `v`s; at higher levels of
  verbosity IRC messages received, HTTP response headers, and information
  regarding resolution of URLs, such as cookies set, can be printed.

## Additional CLI tools

The crate comes with additional binary tools, to aid in testing URL title
retrieval.

### `url-bot-get`

Performs title retrieval using the same code that is used in the bot, but
instead supplied with URLs via the command line, with tweakable request
parameters, such as user agent, and others. It is intended to be useful for
debugging cases where title retrieval fails for some reason, to assist in
offline development.

## IRC

There are IRC channels on [~~Moznet~~](https://wiki.mozilla.org/IRC),
[Freenode](https://freenode.net/) and [OFTC](https://www.oftc.net/),
`#url-bot-rs`.
