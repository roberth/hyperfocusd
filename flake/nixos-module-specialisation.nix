# Tests in: ./vm-test-specialisation.nix
{ self, ... }:

{
  _class = "flake";

  flake.nixosModules.specialisation = { config, lib, pkgs, ... }:
    let
      cfg = config.services.hyperfocusd;
    in
    {
      options.services.hyperfocusd.specialisation = {
        enable = lib.mkOption {
          type = lib.types.bool;
          default = config.services.hyperfocusd.enable;
          defaultText = lib.literalExpression "config.services.hyperfocusd.enable";
          description = ''
            Whether to enable automatic hyperfocus specialisation management.

            When enabled, creates a specialisation.hyperfocus configuration that
            the system switches to when entering hyperfocus mode, and switches
            back from when exiting.
          '';
        };
      };

      config = lib.mkIf cfg.specialisation.enable {
        # Enable the service (default already does this, but being explicit)
        services.hyperfocusd.enable = lib.mkDefault true;

        # Create marker file to identify normal configuration
        environment.etc."example-hyperfocus".text = "false";

        # Create hyperfocus specialisation
        specialisation.hyperfocus.configuration = {
          # Override marker file to identify hyperfocus configuration
          environment.etc."example-hyperfocus".text = lib.mkForce "true";
        };

        # Configure hooks to switch between specialisations
        services.hyperfocusd.settings.hooks = {
          startFocus.argv = [
            "${pkgs.bash}/bin/sh"
            "-c"
            ''
              # Save current system as indirect GC root
              ${config.nix.package}/bin/nix-store --realise /run/current-system --add-root /run/stop-focus-system --indirect
              # Switch to hyperfocus specialisation
              /run/current-system/specialisation/hyperfocus/bin/switch-to-configuration test
            ''
          ];
          stopFocus.argv = [
            "${pkgs.bash}/bin/sh"
            "-c"
            ''
              # Switch back to saved system
              /run/stop-focus-system/bin/switch-to-configuration test
            ''
          ];
        };
      };
    };
}
