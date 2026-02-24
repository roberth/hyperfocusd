{
  description = "Benchmark environment switch daemon";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    nix-cargo-integration.url = "github:yusdacra/nix-cargo-integration";
  };

  outputs = inputs@{ flake-parts, nix-cargo-integration, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        nix-cargo-integration.flakeModule
        ./flake/nixos-module.nix
        ./flake/nixos-module-specialisation.nix
        ./flake/vm-test.nix
        ./flake/vm-test-specialisation.nix
      ];

      systems = [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ];

      perSystem = { config, pkgs, ... }: {
        nci.projects.hyperfocusd.path = ./.;
        nci.crates.hyperfocusd.drvConfig.mkDerivation = {
          postInstall = ''
            ln -s $out/bin/hyperfocusd $out/bin/hyperfocus-on
          '';
        };

        packages.default = config.nci.outputs.hyperfocusd.packages.release;

        devShells.default = config.nci.outputs.hyperfocusd.devShell;
      };

      # raw flake output attrs
      flake = {
        herculesCI.ciSystems = [ "x86_64-linux" ];
      };
    };
}
