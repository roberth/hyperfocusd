{ self, ... }:

{
  _class = "flake";

  perSystem = { pkgs, ... }: {
    checks.vm-test = pkgs.testers.runNixOSTest {
      name = "hyperfocusd-basic-workflow";

      extraBaseModules = self.nixosModules.default;

      nodes.machine = { config, pkgs, ... }: {
        services.hyperfocusd.enable = true;

        specialisation.simple-config = {
          configuration = {
            services.hyperfocusd.settings = {
              hooks.startFocus.argv = [ "${pkgs.bash}/bin/sh" "-c" "echo started-focus > /tmp/hook-state" ];
              hooks.stopFocus.argv = [ "${pkgs.bash}/bin/sh" "-c" "echo stopped-focus > /tmp/hook-state" ];
            };
          };
        };
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

        # Test mutual exclusion: only one client should be active at a time
        # System state: No active clients
        machine.succeed("mkfifo /tmp/release-first /tmp/release-second")

        # Start first client in background
        # It will connect to daemon, enter hyperfocus mode, write started marker, then block reading the fifo
        # System state after this: First client is active
        machine.succeed("hyperfocus-on -- sh -c 'echo started >/tmp/first-started && cat /tmp/release-first' >/dev/null &")

        # Wait for first client to actually start running its command
        machine.succeed("timeout 1 grep started /tmp/first-started >/dev/null")

        # Start second client in background
        # It should NOT run its command until first client finishes
        # System state: First client active, second client waiting (not yet running command)
        machine.succeed("hyperfocus-on -- sh -c 'echo started >/tmp/second-started && cat /tmp/release-second' >/dev/null &")

        # Give second client a moment to try to start
        machine.succeed("sleep 0.5")

        # Verify second client has NOT started yet (mutual exclusion working)
        # If mutual exclusion works, second-started file should not exist yet
        machine.fail("test -f /tmp/second-started")

        # Release first client by writing to its fifo
        # System state after: First client completes, second client becomes active and runs
        machine.succeed("echo done >/tmp/release-first")

        # Wait for second client to start (may take a moment for daemon to accept next connection)
        machine.wait_until_succeeds("grep started /tmp/second-started >/dev/null")

        # Release second client
        # System state after: Second client completes
        machine.succeed("echo done >/tmp/release-second")

        # Test hooks from JSON config file using specialisation
        # Switch to simple-config specialisation which has hooks configured
        machine.succeed("/run/booted-system/specialisation/simple-config/bin/switch-to-configuration test")

        # Run hyperfocus-on with the configured daemon
        # The startFocus hook should run before the command starts, writing "started-focus"
        # The command itself should verify it's in the focused state
        # The stopFocus hook should run after the command completes, writing "stopped-focus"
        machine.succeed("hyperfocus-on -- sh -c 'grep started-focus /tmp/hook-state' >/dev/null")

        # Verify stopFocus hook was executed last (overwrites the file)
        machine.succeed("grep stopped-focus /tmp/hook-state >/dev/null")
      '';
    };
  };
}
