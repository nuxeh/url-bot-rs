{ pkgs ? import <nixpkgs> }:
pkgs.mkShell {
    buildInputs = with pkgs; [
      rustc
      cargo
      pkgconfig
      openssl
      sqlite
    ];
}
