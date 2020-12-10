# Use latest nightly Rust
#
# To use, clone the Mozilla overlay, and provide the path at nix-shell
# invocation, e.g.:
#
#     git clone https://github.com/mozilla/nixpkgs-mozilla.git
#     nix-shell nightly.nix -I rustoverlay=/path/to/overlay
#

with import <nixpkgs> {};
with import <rustoverlay/rust-overlay.nix> pkgs pkgs;

stdenv.mkDerivation {
  name = "url-bot-rs";

  buildInputs = [
    latest.rustChannels.nightly.rust
    pkgconfig
    openssl
    sqlite
  ];
}
