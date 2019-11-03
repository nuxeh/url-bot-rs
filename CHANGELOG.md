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
