{ self, ... }:

{
  _class = "flake";

  perSystem = { pkgs, ... }: {
    checks.vm-test = pkgs.testers.runNixOSTest {
      name = "hyperfocusd-basic-workflow";

      extraBaseModules = self.nixosModules.default;

      nodes.machine = { config, pkgs, ... }: {
        services.hyperfocusd.enable = true;
      };

      testScript = ''
        start_all()

        # Test basic workflow from README:
        # 1. User initiates a benchmark
        # 2. hyperfocus-on sends message to hyperfocusd socket and waits
        # 3. hyperfocusd configures system for benchmarking
        # 4. hyperfocus-on receives response and starts command
        # 5. hyperfocus-on waits for child to finish
        # 6. hyperfocusd configures system back to normal

        machine.wait_for_unit("multi-user.target")
        machine.wait_for_unit("hyperfocusd.socket")

        # Run a simple command through hyperfocus-on
        # Should set HYPERFOCUSING=1 environment variable
        result = machine.succeed("hyperfocus-on -- printenv HYPERFOCUSING")
        assert result.strip() == "1", f"Expected HYPERFOCUSING=1, got {result}"

        # Verify the command ran successfully
        output = machine.succeed("hyperfocus-on -- echo 'test output'")
        assert "test output" in output, f"Expected 'test output' in output, got {output}"

        # Verify daemon is still running after session ends
        machine.succeed("systemctl is-active hyperfocusd.service")
      '';
    };
  };
}
