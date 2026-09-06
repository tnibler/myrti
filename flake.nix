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
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    fenix,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      vipsOverlay = final: prev: {
        vips = prev.vips.overrideAttrs (old: {
          libjpeg = prev.mozjpeg;
        });
      };
      pkgs = import nixpkgs {
        inherit system;
        overlays = [vipsOverlay];
      };
      inherit (pkgs) lib;

      pnpmDeps = pkgs.fetchPnpmDeps {
        pname = "myrti-web-pnpm-deps";
        version = "0.1";
        src = ./web;
        pnpm = pkgs.pnpm_11;
        fetcherVersion = 3; # See https://nixos.org/manual/nixpkgs/stable/#javascript-pnpm-fetcherVersion
        hash = "sha256-Wdoh3V3jTs0zipybwK2rYd4RoRoZ5mOKt1zuHaNTQxc=";
      };

      myrtiWeb = pkgs.stdenv.mkDerivation {
        pname = "myrti-web";
        version = "0.1";
        src = ./web;
        inherit pnpmDeps;

        nativeBuildInputs = [
          pkgs.nodejs
          pkgs.pnpmConfigHook
          pkgs.pnpm_11
        ];

        buildPhase = ''
          pnpm build
        '';

        installPhase = ''
          cp -r dist $out
          cp -r public/shaka-player $out
        '';
      };

      craneLib = (crane.mkLib pkgs).overrideToolchain fenix.packages.${system}.stable.toolchain;
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
          exiftool
          gpac
          rust-jemalloc-sys-unprefixed
        ];

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
          nativeBuildInputs = with pkgs; [clang pkg-config llvmPackages.libclang makeBinaryWrapper];

          cargoToml = ./server/server/Cargo.toml;
          src = lib.fileset.toSource {
            root = ./server;
            fileset = lib.fileset.unions [
              ./server/Cargo.toml
              ./server/Cargo.lock
              (craneLib.fileset.commonCargoSources ./server/myrti-data)
              (craneLib.fileset.commonCargoSources ./server/myrti-core)
              (craneLib.fileset.commonCargoSources ./server/server)
              (lib.fileset.fileFilter (file: file.hasExt "sql" || file.hasExt "toml") ./server/myrti-data)
              (lib.fileset.fileFilter (file: file.hasExt "c" || file.hasExt "h") ./server/myrti-core)
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
      };

      apps = {
        server = let
          runServer = pkgs.writeShellScriptBin "run-server" ''
            export PATH="${pkgs.lib.makeBinPath [pkgs.ffmpeg pkgs.exiftool pkgs.gpac]}:$PATH"
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [pkgs.glib pkgs.vips pkgs.rust-jemalloc-sys-unprefixed]}"
            exec ${server}/bin/server --serve-static ${myrtiWeb} "$@"
          '';
        in {
          type = "app";
          program = "${runServer}/bin/run-server";
        };
        print-openapi = flake-utils.lib.mkApp {
          drv = server;
          name = "print-openapi";
        };
      };

      devShells.default = craneLib.devShell {
        # Inherit inputs from checks.
        checks = self.checks.${system};

        inputsFrom = [server];

        # Extra inputs can be added here; cargo and rustc are provided by default.
        packages = with pkgs; [
          pnpm
          cargo-hakari
          rust-analyzer
          nodejs
          svelte-language-server
          typescript-language-server
          vscode-langservers-extracted
          prettier
          diesel-cli
          sqlite
        ];

        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [pkgs.glib pkgs.vips pkgs.rust-jemalloc-sys-unprefixed];
        LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
      };
    });
}
