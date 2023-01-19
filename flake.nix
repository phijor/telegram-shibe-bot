{
  description = "A Telegram bot for your Shiba Inu needs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = {
    nixpkgs,
    crane,
    flake-utils,
    rust-overlay,
    ...
  }:
  # NB: temporarily skip aarch64-darwin since QEMU can't build there on nixpkgs-unstable
    flake-utils.lib.eachSystem ["aarch64-linux" "x86_64-darwin" "x86_64-linux"] (localSystem: let
      # Replace with the system you want to build for
      crossSystem = "aarch64-linux";

      pkgs = import nixpkgs {
        inherit crossSystem localSystem;
        overlays = [(import rust-overlay)];
      };

      rustToolchain = pkgs.pkgsBuildHost.rust-bin.stable.latest.default.override {
        targets = ["aarch64-unknown-linux-gnu"];
      };

      craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

      # Assuming the above expression was in a file called myCrate.nix
      # this would be defined as:
      # my-crate = pkgs.callPackage ./myCrate.nix { };
      telegram-shibe-bot = pkgs.callPackage ./telegram-shibe-bot.nix {
        inherit craneLib;
      };
    in {
      checks = {
        inherit telegram-shibe-bot;
      };

      packages = {
        inherit telegram-shibe-bot;
        default = telegram-shibe-bot;
      };

      apps.default = flake-utils.lib.mkApp {
        drv = pkgs.writeScriptBin "telegram-shibe-bot" ''
          ${pkgs.pkgsBuildBuild.qemu}/bin/qemu-aarch64 ${telegram-shibe-bot}/bin/cross-rust-overlay
        '';
      };
    });
}
