{ pkgs ? import <nixpkgs> }:
pkgs.mkShell {
    buildInputs = [
      rustc
      cargo
      pkgconfig
      openssl
      sqlite
    ];
}
