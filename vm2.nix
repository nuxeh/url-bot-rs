# Example of NixOS configuraion for deployment, or for testing using
# `nixos-shell`

{ config, pkgs, ... }:

{
  boot.kernelPackages = pkgs.linuxPackages_latest;
  services.openssh.enable = true;

  services.url-bot-rs = {
    enable = true;
    server = "irc.nomnomnomnom.co.uk";
    settings = {
      "default" = {
        "plugins" = {
          "imgur" = {
            "api_key" = "";
          };
          "youtube" = {
            "api_key" = "";
          };
          "vimeo" = {
            "api_key" = "";
          };
        };
        "network" = {
          "name" = "default";
          "enable" = true;
        };
        "features" = {
          "report_metadata" = false;
          "report_mime" = false;
          "mask_highlights" = false;
          "send_notice" = false;
          "history" = false;
          "cross_channel_history" = false;
          "invite" = false;
          "autosave" = false;
          "send_errors_to_poster" = false;
          "reply_with_errors" = false;
          "partial_urls" = false;
          "nick_response" = false;
          "reconnect" = false;
        };
        "parameters" = {
          "url_limit" = 10;
          "status_channels" = [];
          "nick_response_str" = "";
          "reconnect_timeout" = 10;
        };
        "http" = {
          "timeout_s" = 10;
          "max_redirections" = 10;
          "max_retries" = 3;
          "retry_delay_s" = 5;
          "accept_lang" = "en";
        };
        "database" = {
          "type" = "in-memory";
        };
        "connection" = {
          "nickname" = "url-bot-rs";
          "nick_password" = "";
          "alt_nicks" = [
            "url-bot-rs_"
          ];
          "username" = "url-bot-rs";
          "realname" = "url-bot-rs";
          "server" = "irc.nomnomnomnom.co.uk";
          "port" = 26067;
          "password" = "chaesachoeGohzah1soo6OhTeefeiloo";
          "use_ssl" = false;
          "channels" = [
            "#url-bot-rs-test"
          ];
          "user_info" = "Feed me URLs.";
        };
      };
    };
  };

  networking.firewall.enable = false;
}
