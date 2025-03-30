{
  description = "Build a cargo workspace";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };

    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    fenix,
    flake-utils,
    advisory-db,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = nixpkgs.legacyPackages.${system};

      inherit (pkgs) lib;

      craneLib = crane.mkLib pkgs;
      src = craneLib.cleanCargoSource ./server;

      # Common arguments can be set here to avoid repeating them later
      commonArgs = {
        inherit src;
        strictDeps = true;

        nativeBuildInputs = with pkgs; [
          pkg-config
          llvmPackages.libclang
          clang
        ];
        buildInputs = with pkgs; [
          vips.dev
          glib.dev
          ffmpeg
          shaka-packager
        ];

        # Additional environment variables can be set directly
        # MY_CUSTOM_VAR = "some value";
        LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
      };

      cargoArtifacts =
        craneLib.buildDepsOnly commonArgs
        // {
          inherit (craneLib.crateNameFromCargoToml ./server/server) name version;
        };

      server = craneLib.buildPackage (commonArgs
        // {
          inherit cargoArtifacts;
          inherit (craneLib.crateNameFromCargoToml {inherit src;}) version;

          # help idk why this isn't inherited from commonArgs
          nativeBuildInputs = with pkgs; [clang pkg-config llvmPackages.libclang];

          cargoToml = ./server/server/Cargo.toml;
          src = lib.fileset.toSource {
            root = ./server;
            fileset = lib.fileset.unions [
              ./server/Cargo.toml
              ./server/Cargo.lock
              (craneLib.fileset.commonCargoSources ./server/myrti_core)
              (craneLib.fileset.commonCargoSources ./server/server)
              (lib.fileset.fileFilter (file: file.hasExt "sql" || file.hasExt "toml" || file.hasExt "sql" || file.hasExt "c" || file.hasExt "h") ./server/myrti_core)
            ];
          };
          doCheck = false;
        });
    in {
      checks = {
        # Build the crates as part of `nix flake check` for convenience
        inherit server;

        # Ensure that cargo-hakari is up to date
        my-workspace-hakari = craneLib.mkCargoDerivation {
          inherit src;
          pname = "my-workspace-hakari";
          cargoArtifacts = null;
          doInstallCargoArtifacts = false;

          buildPhaseCargoCommand = ''
            cargo hakari generate --diff  # workspace-hack Cargo.toml is up-to-date
            cargo hakari manage-deps --dry-run  # all workspace crates depend on workspace-hack
            cargo hakari verify
          '';

          nativeBuildInputs = [
            pkgs.cargo-hakari
            pkgs.pkg-config
          ];
        };
      };

      packages = {
        inherit server;

        web = pkgs.buildNpmPackage {
          name = "myrti-web";
          src = ./web;
          npmDepsHash = "sha256-SW4FKbsZyd4S/UQw+bh9rUV2LbfK3kYx175H4nV9yiI=";
          installPhase = ''
            cp -r dist $out/
          '';
        };
      };

      apps = {
        server = flake-utils.lib.mkApp {
          drv = server;
          name = "server";
        };
        print-openapi = flake-utils.lib.mkApp {
          drv = server;
          name = "print-openapi";
        };
      };

      devShells.rust = craneLib.devShell {
        # Inherit inputs from checks.
        checks = self.checks.${system};

        # Additional dev-shell environment variables can be set directly
        # MY_CUSTOM_DEVELOPMENT_VAR = "something else";
        inputsFrom = [server];

        # Extra inputs can be added here; cargo and rustc are provided by default.
        packages = with pkgs; [
          cargo-hakari
          rust-analyzer
        ];

        LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
      };

      devShells.web = pkgs.mkShell {
        packages = with pkgs; [
          nodejs_23
          svelte-language-server
          typescript-language-server
        ];
      };
    });
}
