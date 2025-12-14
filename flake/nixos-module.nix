{ self, ... }:

{
  _class = "flake";

  flake.nixosModules.default = { ... }: {
    imports = [ self.nixosModules.service ];
  };

  flake.nixosModules.service = { config, lib, pkgs, ... }:
    let
      cfg = config.services.hyperfocusd;
    in
    {
      options.services.hyperfocusd = {
        enable = lib.mkEnableOption "hyperfocusd benchmark environment switch daemon";

        package = lib.mkOption {
          type = lib.types.package;
          default = self.packages.${pkgs.system}.default;
          description = "The hyperfocusd package to use";
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
            ExecStart = "${cfg.package}/bin/hyperfocusd daemon";
            Type = "notify";
            NotifyAccess = "main";
          };
        };

        environment.systemPackages = [ cfg.package ];
      };
    };
}
