# Plan: a `flake.nix` for colourme

Goal: make colourme consumable as a **build input** by `nixos-config` (the
strategy-A default render in `nixos-config/docs/THEME-PIPELINE.md`), plus a dev
shell and checks. It must build hermetically from a pinned lockfile and not need
installing on the host.

## Requirements

- `packages.<system>.default` = `colourme` (so `inputs.colourme.packages.${system}.default`
  works), also aliased as `packages.<system>.colourme`.
- Hermetic build from `Cargo.lock`; no network at build time.
- `devShells.<system>.default` with a toolchain matching the repo's pin
  (`rust 1.96.0` in `.tool-versions`).
- `checks.<system>` running `cargo test`, `cargo clippy`, `cargo fmt --check`;
  `nix flake check` green.
- `formatter.<system>` for the Nix files.
- Multi-system: at least `x86_64-linux` (NixOS host) and `aarch64-darwin`
  (upstream dev machine).
- Consumable from `nixos-config` with `inputs.colourme.inputs.nixpkgs.follows =
  "nixpkgs"`.

## The one hard problem: the `colour_utils` path dependency

`Cargo.toml` declares:

```toml
colour_utils = { path = "../colour_utils" }
```

`colour_utils` is a **separate GitHub repo** (`lukamanitta/colour_utils`,
v0.1.0, only dep `regex`), with no flake. nixpkgs' `cargoLock`
(`importCargoLock`) **cannot represent path dependencies** — its vendoring runs
against `src` alone, where `../colour_utils` does not exist, so
`cargoLock.lockFile = ./Cargo.lock` does not work as-is. Pick one resolution:

### Option A (recommended) — combined source tree + `cargoHash`

Keep the path dep exactly as it is (zero change to the dev workflow). Assemble a
source tree where the two crates are siblings and point cargo into it:

```
<src>/
  colourme/      ← the colourme repo
  colour_utils/  ← the sibling
```

then build with `cargoRoot = "colourme"`; `../colour_utils` resolves inside the
tree and the legacy `cargoHash` vendoring (`cargo vendor` over the whole tree)
picks it up. `colour_utils` is supplied as a `flake = false` input.

- Pros: no `Cargo.toml`/`Cargo.lock` churn; both crates stay co-developable;
  no tags/publishing needed.
- Cons: a single `cargoHash` must be refreshed whenever *any* dependency
  changes (vs per-lockfile hashes); the flake couples the two repos via inputs.

Sketch (see full flake below):

```nix
src = pkgs.runCommand "colourme-src" { } ''
  mkdir -p $out
  cp -r ${colourmeSrc} $out/colourme
  cp -r ${colourUtilsSrc} $out/colour_utils
'';
cargoRoot = "colourme";
cargoHash = lib.fakeHash; # nix build reports the real value
```

### Option B — make `colour_utils` a git dependency + `cargoLock`

Uncomment the line already in `Cargo.toml`:

```toml
colour_utils = { git = "https://github.com/lukamanitta/colour_utils.git", tag = "v0.1.0" }
```

regenerate `Cargo.lock`, then use `cargoLock.lockFile = ./Cargo.lock` with
`cargoLock.outputHashes = { "colour_utils-0.1.0" = "sha256-…"; }`.

- Pros: lockfile-based, no combined tree, the "standard" shape.
- Cons: changes the manifest; local co-development needs an override in a
  **gitignored** `.cargo/config.toml`:
  ```toml
  [patch."https://github.com/lukamanitta/colour_utils.git"]
  colour_utils = { path = "../colour_utils" }
  ```
  Caveat: while the patch is active cargo rewrites `Cargo.lock` to the path
  source, so it must not be committed in that state. Needs a tag/rev pin.

### Option C — publish `colour_utils` to crates.io

`colour_utils = "0.1"` + plain `cargoLock`. Cleanest build, needs a release
workflow. Likely overkill for now.

### Option D — make `colour_utils` a git submodule of colourme

Move it inside the colourme repo at `./colour_utils` (submodule), change the dep
to `path = "colour_utils"`. Then the standard `cargoLock` works and no extra
input is needed. Bigger workflow change (nested repo); only if you want the
crates to live together.

**Recommendation: Option A** now (no dev churn, both crates actively changing).
Switch to B/C once `colour_utils` has tagged releases.

## Toolchain

`.tool-versions` pins `rust 1.96.0`. Two choices:

1. **`oxalica/rust-overlay` + `rust-toolchain.toml`** (recommended): add
   ```toml
   [toolchain]
   channel = "1.96.0"
   components = ["rustfmt", "clippy"]
   ```
   and use `pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml`. One
   source of truth across mise and Nix; build uses the same rustc you develop
   with. (mise can read `rust-toolchain.toml`, so the rust line can leave
   `.tool-versions`.)
2. **nixpkgs `rustPlatform` default rustc.** Simpler (no extra input) but the
   build's rustc drifts from the mise pin.

Recommendation: 1, since the repo already pins a version and drift here is the
classic "works locally, fails in CI/build" trap. Falls back to 2 if you want the
absolute minimum flake.

## Proposed `flake.nix` (sketch)

```nix
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

  outputs = { self, nixpkgs, colour-utils, rust-overlay }:
    let
      lib = nixpkgs.lib;
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];

      eachSystem = f: lib.genAttrs systems (system:
        f (import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        }));

      # Only the files cargo needs; avoids dragging in ./target.
      colourmeSrc = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./src
        ];
      };
    in
    {
      packages = eachSystem (pkgs:
        let
          rust = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override {
            extensions = [ "rust-src" ];
          };

          colourme = pkgs.rustPlatform.buildRustPackage {
            pname = "colourme";
            version = "0.1.0";

            src = pkgs.runCommand "colourme-src" { } ''
              mkdir -p $out
              cp -r ${colourmeSrc} $out/colourme
              cp -r ${colour-utils} $out/colour_utils
            '';
            cargoRoot = "colourme";
            cargoHash = lib.fakeHash; # replace with the real hash

            nativeBuildInputs = [ rust ];
            meta.mainProgram = "colourme";
          };
        in
        {
          inherit colourme;
          default = colourme;
        });

      devShells = eachSystem (pkgs:
        let rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        in {
          default = pkgs.mkShell {
            packages = [ rust ];
            inputsFrom = [ self.packages.${pkgs.stdenv.hostPlatform.system}.colourme ];
          };
        });

      formatter = eachSystem (pkgs: nixpkgs.legacyPackages.${pkgs.system}.nixfmt-rfc-style);

      checks = eachSystem (pkgs: {
        build = self.packages.${pkgs.system}.default;
        # fmt/clippy/test as derivations that run the toolchain here
      });

      overlays.default = final: prev: {
        colourme = self.packages.${final.stdenv.hostPlatform.system}.default;
      };
    };
}
```

Notes for the implementer:

- `lib.fileset.toSource` requires a modern nixpkgs (fine on unstable).
- `cargoHash = lib.fakeHash` → run `nix build`, copy the real hash in.
- If Option A's `cargoRoot` + `cargoHash` vendoring misbehaves, fall back to
  Option B. Verify before wiring into `nixos-config`.
- `meta.mainProgram = "colourme"` so `nix run` works.
- Commit the generated `flake.lock`.

## How `nixos-config` will consume it

```nix
# nixos-config/flake.nix
inputs.colourme.url = "github:lukamanitta/colourme";
inputs.colourme.inputs.nixpkgs.follows = "nixpkgs";
```

Then the default-render derivation in `THEME-PIPELINE.md` uses
`inputs.colourme.packages.${pkgs.system}.default` as its `nativeBuildInputs`
(never added to `home.packages`). `follows` keeps a single nixpkgs in the
closure; alternatively consume `inputs.colourme.overlays.default`.

## Checks / CI (optional but cheap)

- `nix build .#default` and run `./result/bin/colourme --version`.
- `nix develop -c cargo test` / `cargo clippy --all-targets -- -D warnings` /
  `cargo fmt --check`.
- `nix flake check` for all systems.
- GitHub Actions matrix (`nix flake check`) once the flake lands.

## Verification before declaring done

1. `nix build .#` → `result/bin/colourme --version` prints `0.1.0`.
2. Reproduce the hermetic render from `THEME-PIPELINE.md` with the built binary:
   `HOME=/homeless-shelter result/bin/colourme --config … --schemes-dir …
   --dest-root $out --no-hooks Gruvbox` and confirm the tree.
3. `nix flake check` passes on `x86_64-linux`.
4. Add the input to `nixos-config` on a branch and confirm the defaults
   derivation builds.

## Open decisions

- Path-dep handling: **A** (recommended) vs B vs C vs D.
- Toolchain: rust-overlay + `rust-toolchain.toml` (recommended) vs nixpkgs rustc.
- Systems list beyond `x86_64-linux` / `aarch64-darwin`.
- Whether to also expose a `lib` helper (e.g. a function that renders a scheme
  into a store path) now, or defer until `nixos-config` needs it.
