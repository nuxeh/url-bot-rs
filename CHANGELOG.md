# v0.3.0

- Retry HTTP requests on server errors.
- Remove logging of errors to database
- Refactor configuration for HTTP. `parameters.accept_lang` is now
  `http.accept_lang` in the configuration.
- Add configuration feature to limit history to same channels

# v0.2.4

- Allow build with bundled, statically linked, SQLite
- Refactor image dimension extraction, and support a couple more formats.
- Resolve #249, relative HTTP redirection paths
- Add Github workflows for CI
- Upgrade dependencies
- Remove chrono dependency
- Search in default search path for configurations if no configurations are
  otherwise specified

# v0.2.3

- Add capability of running multiple instances, to connect to multiple IRC
  networks
- Remove `--db` command line parameter
- Add `--conf-dir` command line parameter, allowing configurations to be
  searched for and loaded from a directory
- Add a configuration file section, `network`, which may be used to specify a
  network name, and to enable or disable it
- Use network name to create a default sqlite db path
- Allow multiple `--conf` command line parameters
- Fix a bug whereby the bot would load any valid TOML as a configuration, using
  default values
- Use an enum underneath for database type in configuration, refactor sqlite
  database path handling
- Add IRC server reconnection
- Improve test coverage
- Update man page

# v0.2.2

- Nick response
- Refactor IRC error reporting
- Improve test coverage

# v0.2.1

- URL de-duplication
- Addition of preliminary Nix files
- Reporting of errors over IRC via PRIVMSG
- Make schemeless URL matching optional (disabled by default)
- Attempt to retrieve titles where a scheme is missing
- Add `/invite` and `/kick` capability
- Adaptively download webpage content until a title is found
- Use scraper/html5ever to parse HTML, rather than a regex
- Log title retrieval failures to database
- Add `url-bot-get` tool
- Cookie support
- Re-add custom user-agent, make user agent non-configurable
- Request "identity" for accept-encoding
- Update dependencies
- Add a logo
- Clippy linting
- Add test coverage in CI
- Add Debian packaging, manual page, systemd integration
- Add --debug and --version command line flags
- Tilde expansion in paths
- Unified configuration and XDG paths
- Ignore tokens containing invalid characters in URLs

# v0.2.0

Initial development
