{ self, ... }:

{
  _class = "flake";

  flake.nixosModules.default = { ... }: {
    imports = [ self.nixosModules.service ];
  };

  flake.nixosModules.service = { config, lib, pkgs, ... }:
    let
      cfg = config.services.hyperfocusd;

      settingsFormat = pkgs.formats.json {};
    in
    {
      options.services.hyperfocusd = {
        enable = lib.mkEnableOption "hyperfocusd benchmark environment switch daemon";

        package = lib.mkOption {
          type = lib.types.package;
          default = self.packages.${pkgs.system}.default;
          description = "The hyperfocusd package to use";
        };

        settings = lib.mkOption {
          type = settingsFormat.type;
          default = {};
          description = ''
            Configuration for hyperfocusd daemon.
            See <link xlink:href="https://github.com/NixOS/rfcs/blob/master/rfcs/0042-config-option.md">RFC 42</link> for details.
          '';
        };
      };

      config = lib.mkIf cfg.enable {
        systemd.sockets.hyperfocusd = {
          description = "Benchmark environment switch daemon socket";
          wantedBy = [ "sockets.target" ];
          socketConfig = {
            ListenStream = "/run/hyperfocusd/hyperfocusd.socket";
            SocketMode = "0666";
            RuntimeDirectory = "hyperfocusd";
            Accept = "no";
          };
        };

        systemd.services.hyperfocusd = {
          description = "Benchmark environment switch daemon";
          requires = [ "hyperfocusd.socket" ];

          serviceConfig = {
            ExecStart =
              if cfg.settings != {} then
                "${cfg.package}/bin/hyperfocusd daemon --config ${settingsFormat.generate "hyperfocusd-config.json" cfg.settings}"
              else
                "${cfg.package}/bin/hyperfocusd daemon";
            Type = "notify";
            NotifyAccess = "main";
          };
        };

        environment.systemPackages = [ cfg.package ];
      };
    };
}
