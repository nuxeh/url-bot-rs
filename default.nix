with import <nixpkgs> {};

stdenv.mkDerivation {
    name = "url-bot-rs";

    buildInputs = [
      pkgconfig
      openssl
      sqlite
    ];
}
