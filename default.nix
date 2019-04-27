let
  pkgs = import <nixpkgs> {
    config = {};
    overlays = [
      (import ./overlay.nix)
    ];
  };
in pkgs.url-bot-rs
