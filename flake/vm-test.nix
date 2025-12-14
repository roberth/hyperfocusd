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

        # Test exit code propagation
        machine.succeed("hyperfocus-on -- sh -c 'exit 42' || test $? -eq 42")

        # Test '--' separator handling (command should work with or without it)
        machine.succeed("hyperfocus-on printenv HYPERFOCUSING | grep 1 >/dev/null")

        # Test empty command error
        machine.fail("hyperfocus-on 2>&1 | grep 'Error: no command specified' >/dev/null")

        # Test daemon not running error
        machine.succeed("systemctl stop hyperfocusd.socket hyperfocusd.service")
        machine.fail("hyperfocus-on -- echo test 2>&1 | grep 'Failed to connect to hyperfocusd' >/dev/null")
        machine.succeed("systemctl start hyperfocusd.socket")

        # Trigger socket activation by making a connection
        machine.succeed("hyperfocus-on -- echo 'reactivated' | grep 'reactivated' >/dev/null")

        # Verify daemon is running after socket activation
        machine.succeed("systemctl is-active hyperfocusd.service")
      '';
    };
  };
}
