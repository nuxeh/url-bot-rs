{
  description = "url-bot-rs";

  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.simpleFlake {
      inherit self nixpkgs;
      name = "simple-flake";
      overlay = ./nix/overlay.nix;
      config = ./nix/url-bot-rs.nix;
      shell = ./nix/shell.nix;
    };
}
