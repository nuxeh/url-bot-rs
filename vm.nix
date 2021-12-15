# Example of NixOS configuraion for deployment, or for testing using
# `nixos-shell`

{ config, pkgs, ... }:

{
  boot.kernelPackages = pkgs.linuxPackages_latest;
  services.openssh.enable = true;

  services.url-bot-rs = {
    enable = true;
    networks = {
      "net" = {
        nickname = "url-bot-rs-test";
        altNicks = [ "url-bot-rs-test_" "url-bot-rs-test__" ];
        server = "chat.freenode.net";
        port = 6697;
        ssl = true;
        channels = [ "#url-bot-rs-test" ];
        features = [ "history" "reconnect" ];
        statusChannels = [ "#url-bot-rs-test" ];
        sqliteDb = true;
      };
      "othernet" = {
        nickname = "url-bot-rs-test";
        altNicks = [ "url-bot-rs-test_" "url-bot-rs-test__" ];
        server = "chat.freenode.net";
        port = 6697;
        ssl = true;
        channels = [ "#url-bot-rs-test" ];
        features = [ "reconnect" ];
        youtubeAPIKey = "fnar123123123";
        imgurAPIKey = "woof123123123";
      };
      "yetanothernet" = {
        enable = false;
        nickname = "url-bot-rs-test";
        altNicks = [ "url-bot-rs-test_" "url-bot-rs-test__" ];
        server = "chat.freenode.net";
        port = 6697;
        ssl = true;
        channels = [ "#url-bot-rs-test" ];
        features = [ "reconnect" ];
      };
    };
  };

  networking.firewall.enable = false;
}
