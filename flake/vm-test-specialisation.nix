# Run with:
#   nix build .#checks.x86_64-linux.vm-test-specialisation
{ self, ... }:

{
  _class = "flake";

  perSystem = { pkgs, ... }: {
    checks.vm-test-specialisation = pkgs.testers.runNixOSTest {
      name = "hyperfocusd-specialisation";

      extraBaseModules = self.nixosModules.default;

      nodes.machine = { config, pkgs, ... }: {
        services.hyperfocusd = {
          specialisation.enable = true;
          # Use debug logging to test that log level configuration works
          settings.log_level = "debug";
        };
      };

      testScript = ''
        start_all()

        machine.wait_for_unit("multi-user.target")
        machine.wait_for_unit("hyperfocusd.socket")

        # Verify we're in the normal (non-hyperfocus) configuration initially
        result = machine.succeed("cat /etc/example-hyperfocus")
        assert result.strip() == "false", f"Should start in normal config, got {result}"

        # Run a command - the command runs in hyperfocus config (after startFocus hook)
        # Verify inside the command that we're in hyperfocus
        machine.succeed("hyperfocus-on -- sh -c 'test $(cat /etc/example-hyperfocus) = true'")

        # After the command completes, stopFocus hook runs asynchronously
        # Wait for the system to switch back to normal config
        machine.wait_until_succeeds("test $(cat /etc/example-hyperfocus) = false")

        # Check journal logs to verify structured logging is working
        logs = machine.succeed("journalctl -u hyperfocusd --no-pager")
        print("=== hyperfocusd journal logs ===")
        print(logs)

        # Verify debug logs are present (log_level is set to "debug" in config)
        assert "Client connected" in logs, "Debug log 'Client connected' not found"
        assert "Executing hook" in logs, "Debug log 'Executing hook' not found"
      '';
    };
  };
}
