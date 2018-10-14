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

    cd url-bot-rs
    cargo build

### Run tests

    cargo test

## Configuration

A configuration file is required to specify IRC server settings for the bot,
this can be specified with `--conf=<path>`. If not provided, a default search
path will be used:

* `./config.toml`

A default configuration is provided as `example.config.toml` in the repository.

This may include:
- Address of the IRC server to connect to
- Connection credentials
- The nick the bot will use when joining the server
- Channels to join on the server

It is also possible to configure a number of optional features for the bot's
operation, specified in the section `[features]`:

- `mask_highlights` (bool) inserts invisible characters to defeat highlight
  regexes
- `send_notice` (bool) causes the bot to respond with notices rather than
  private messages
- `report_metadata` (bool) if enabled, causes image metadata to be reported
- `report_mime` (bool) if enabled, causes mime types to be reported, if no
  other title or metadata is found.

A sqlite database may be provided, by specifying a path with `--db=<path>`. If
this option is given, the bot will initialise the database if it doesn't
already exist, and log all URLs posted to it. If the same URL is posted again,
information about the previous posting is added to the returned message.

## Run

    cargo run

## Install

    cargo install

After this, the bot may be started by running `url-bot`

Usage is printed by providing `--help` on run.
