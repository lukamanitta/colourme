{
  description = "colourme — render colour schemes into config files";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    colour-utils = {
      url = "github:lukamanitta/colour_utils";
      flake = false;
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      colour-utils,
      rust-overlay,
      ...
    }:
    let
      lib = nixpkgs.lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      eachSystem =
        f:
        lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ (import rust-overlay) ];
            }
          )
        );

      # Only the files cargo needs; avoids dragging in ./target and docs.
      colourmeSrc = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./src
        ];
      };

      version = "0.1.0";
    in
    {
      packages = eachSystem (
        pkgs:
        let
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          # Option A: colour_utils stays a sibling path dependency. Assemble a
          # tree where `../colour_utils` resolves, then vendored deps come from
          # cargoHash over the whole tree.
          src = pkgs.runCommand "colourme-src" { } ''
            mkdir -p $out
            cp -r ${colourmeSrc} $out/colourme
            cp -r ${colour-utils} $out/colour_utils
          '';

          colourme = rustPlatform.buildRustPackage {
            pname = "colourme";
            inherit version src;

            cargoRoot = "colourme";
            buildAndTestSubdir = "colourme";
            cargoHash = "sha256-YJUJocLaqHebjb3vOcLxaZ7Dh966m48LGnCoTcjId/M=";

            nativeBuildInputs = [ rustToolchain ];

            meta = {
              description = "Render colour scheme templates into config files";
              mainProgram = "colourme";
            };
          };
        in
        {
          inherit colourme;
          default = colourme;
        }
      );

      devShells = eachSystem (
        pkgs:
        let
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        in
        {
          default = pkgs.mkShell {
            packages = [ rustToolchain ];
            inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.colourme ];
          };
        }
      );

      formatter = eachSystem (pkgs: pkgs.nixfmt-rfc-style);

      checks = eachSystem (
        pkgs:
        let
          inherit (self.packages.${pkgs.stdenv.hostPlatform.system}) colourme;

          # Reuse the build's vendored deps/toolchain but skip compilation and
          # just run the requested cargo command.
          mkCargoCheck =
            name: command:
            colourme.overrideAttrs (_: {
              pname = "colourme-check-${name}";
              dontCargoBuild = true;
              buildPhase = ":";
              doCheck = true;
              checkPhase = ''
                runHook preCheck
                pushd colourme
                ${command}
                popd
                runHook postCheck
              '';
              installPhase = "mkdir -p $out";
              dontFixup = true;
            });
        in
        {
          # buildRustPackage runs `cargo test` in its check phase.
          build = colourme;
          clippy = mkCargoCheck "clippy" "cargo clippy --offline --all-targets -- -D warnings";
          fmt = mkCargoCheck "fmt" "cargo fmt --check";
        }
      );

      overlays.default = final: prev: {
        colourme = self.packages.${final.stdenv.hostPlatform.system}.default;
      };
    };
}
