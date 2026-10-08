# Planned interface changes

Spec for the CLI/behaviour changes needed to make `colourme` a clean seam for a
NixOS + Home Manager setup. The Nix side wants three things we can't do today:

1. **A portable config** that works unchanged whether it lives at
   `~/.config/colourme` (live) or at a `/nix/store/...` path (build-time).
2. **A hermetic render mode** — no side effects, no hooks, outputs redirected
   into a scratch directory — so a derivation can generate default theme files.
3. **Robust writes** — create missing directories, never leave a half-written
   config, sane CLI.

Everything below preserves the current live UX: `colourme <scheme>` with no
flags behaves exactly as it does now. All changes are additive.

See `REVIEW.md` for the bugs/robustness findings these are partly derived from.

## Target CLI

```
colourme [OPTIONS] <scheme>
colourme [OPTIONS] list
```

| Flag | Argument | Meaning |
|---|---|---|
| `--config` | `PATH` | Config TOML to use (default: XDG `colourme/config.toml`) |
| `--schemes-dir` | `DIR` | Directory of `<scheme>.toml` files (default: XDG `colourme/schemes`) |
| `--dest-root` | `DIR` | Re-root `$HOME`-based destinations under `DIR` |
| `--no-hooks` | — | Render + write, but run no `post_hook` commands |
| `--dry-run` | — | Resolve everything, run nothing, write nothing; print planned output |
| `-h`, `--help` | — | Usage, exit 0 |
| `-V`, `--version` | — | Version, exit 0 |

Env vars (lower precedence than flags): `COLOURME_CONFIG`,
`COLOURME_SCHEMES_DIR`.

Resolution order for each input path: **flag > env var > XDG default**.

XDG default base: `$XDG_CONFIG_HOME` if set and non-empty, otherwise
`$HOME/.config`. (This fixes the current hardcoded `~/.config/colourme`.)

Exit codes: `0` success, `1` runtime error (I/O, parse, render), `2` usage
error (bad flag/arity).

Implement the parser with `clap` (derive) if adding a dependency is acceptable;
otherwise hand-roll it, but `--help`/unknown-flag handling must be correct
(current code treats `--help` as a scheme name).

---

## 1. Create parent directories on write

**Today:** `write_output` calls `OpenOptions::open`, which fails if the
destination's parent directory is missing. The Nix side would otherwise have to
scaffold empty directories with dummy files.

**Behaviour:** before opening/renaming the destination, `fs::create_dir_all` on
its parent. If the parent is empty (a bare filename), skip. On failure, return a
`String` error naming the directory.

**Edge cases:**
- parent exists → no-op.
- parent is a file → error from `create_dir_all`, surfaced with the path.
- applies to `--dest-root`-rewritten paths too.

**Test:** destination `tmp/a/b/c/out.txt` with no pre-existing dirs renders
successfully and creates `a/b/c`.

## 2. Configurable input paths

**Today:** `default_config_path()` and `default_colourscheme_path()` hardcode
`~/.config/colourme` and ignore `XDG_CONFIG_HOME`.

**Behaviour:**
- `--config PATH` / `COLOURME_CONFIG` select the config file.
- `--schemes-dir DIR` / `COLOURME_SCHEMES_DIR` select the scheme directory;
  the scheme file is `DIR/<scheme>.toml`.
- With none set, use the XDG defaults above.
- Paths given via flag/env are still `~`-expanded (see §7).
- A scheme argument is always a name, never a path.

**Test:** `COLOURME_CONFIG=/tmp/c.toml colourme X` uses `/tmp/c.toml`; a flag
overrides the env var; unset env falls back to XDG.

## 3. `--no-hooks`

**Today:** every entry's `post_hook` runs unconditionally via `sh -c`, and
failures are swallowed (`REVIEW.md` O1). A derivation can't run `hyprctl`.

**Behaviour:** with `--no-hooks`, skip all `post_hook` execution. Log
`[<name>] skipping post-hook (--no-hooks)` per entry (or stay quiet — pick one
and be consistent). Files are still rendered and written.

**Note:** `--no-hooks` neither implies nor is implied by `--dry-run`.
`--dry-run` implies no hooks *and* no writes.

**Test:** config with `post_hook` that would create a sentinel file; with
`--no-hooks` the sentinel is absent, the destination is written.

## 4. `--dest-root DIR`

**Today:** destinations are arbitrary absolute/`~` paths written verbatim. To
generate a self-contained default tree in a store path, we must redirect them.

**Behaviour:** let `home` be `$HOME` with any trailing slash removed, and let
`root` be `--dest-root` (relative roots resolve against CWD).

For each entry, after `~` expansion of `destination`:
- if `dest == home` → new path is `root`.
- else if `dest` starts with `home + "/"` → new path is
  `root + "/" + dest[len(home)+1..]`.
- else (destination not under `$HOME`) → leave the destination **unchanged**.

So a config declaring:

```toml
destination = "~/.config/hypr/theme/colourme.lua"
```

rendered with `--dest-root /out` writes `/out/.config/hypr/theme/colourme.lua`.

Destinations outside `$HOME` are intentionally left alone (rather than erroring)
so a config can, in principle, target an absolute path in both modes; document
that heading into a sandbox with such a destination will fail at write time.

`--dest-root` does not change `template` resolution (see §7) and does not imply
`--no-hooks`.

**Test:** `~/.config/x` → `<root>/.config/x`; `/etc/x` unchanged; `~` → `<root>`.

## 5. Atomic writes (and symlink policy)

**Today:** `write_output` truncates then writes, so a crash/full disk leaves a
corrupt file that a running program may read. It also writes *through* symlinks
(the open resolves the link), which is a feature for dotfile setups.

**Behaviour:** keep the "follow symlinks" semantics but make the write atomic:
1. Resolve the destination's symlink chain to a real target path. If the
   destination (or any link) does not exist, the target is the destination path
   itself.
2. `create_dir_all` the target's parent (§1).
3. Write the content to a temp file **in the same directory** as the target
   (e.g. `.colourme.<pid>.<rand>.tmp`).
4. `rename(temp, target)` over the target.

This yields: write-through preserved (links are followed before the rename),
no partially-written file ever visible, same-filesystem rename guaranteed.

**Edge cases:**
- Destination is a dangling symlink → resolve to its target, create that
  parent, write the target.
- Destination is a directory → error.
- Rename across filesystems is impossible by construction (temp in same dir).
- On any failure, remove the temp file before returning the error.
- Consider `fsync` on the temp file before rename (optional; note the choice).

**Test:** writing to a symlink updates the link target and leaves the symlink
intact; a write failure (e.g. read-only dir) leaves the old content untouched
and no temp file behind.

## 6. CLI hygiene

**Today:** exactly one positional arg; no flags; no way to list schemes.

**Behaviour:**
- `-h`/`--help` prints usage listing all flags, exits 0.
- `-V`/`--version` prints the crate version, exits 0.
- `colourme list` prints the available scheme names — files in `--schemes-dir`
  ending in `.toml`, with the extension stripped, sorted; one per line;
  excludes `_template.toml` (any leading `_`); exit 0. Missing schemes dir →
  error, exit 1.
- `--dry-run`: perform full resolution + rendering for every entry, run no
  hooks, write nothing, and print for each entry something like
  `[<name>] would write <dest> (<N> bytes)`. Errors still reported per the
  normal path. Exit 0 if all entries resolve.
- Unknown flag or wrong arity → usage message on stderr, exit 2.
- A scheme name that doesn't exist → current clear "Couldn't read file ..."
  error, exit 1.

**Test:** `--help` exit 0; `--version` matches `Cargo.toml`; `list` output is
sorted/ext-less; `--dry-run` creates no files and runs no hooks.

## 7. Relative path resolution

**Goal:** one config that is portable between the live XDG location and a store
path. `templates/` sit next to `config.toml`; a relative `template` should
resolve relative to the config file, not the process CWD.

**Behaviour:** define "absolute" = starts with `/`; "tilde" = starts with `~`.
- `template`:
  - tilde → expand with `$HOME` (as today).
  - absolute → use as-is.
  - otherwise (relative) → resolve against the **directory containing the
    config file**.
- `destination`:
  - tilde → expand with `$HOME` (then §4 may re-root).
  - absolute → use as-is.
  - otherwise (relative) → resolve against `--dest-root` if set, else against
    CWD.
- `post_hook` strings are unchanged (not path-resolved).

This lets the config say `template = "templates/hypr.lua"` and be correct
whether `config.toml` is at `~/.config/colourme/config.toml` or
`/nix/store/.../config.toml`.

**Test:** a config in `tmp/cfg/config.toml` with `template = "t/hypr.lua"`
resolves to `tmp/cfg/t/hypr.lua` regardless of the process CWD.

---

## Worked examples

Live apply (unchanged semantics):

```bash
colourme Gruvbox
```

Hermetic build-time render (what the Nix derivation will call):

```bash
colourme \
  --config "$src/config.toml" \
  --schemes-dir "$src/schemes" \
  --dest-root "$out" \
  --no-hooks \
  Gruvbox
# writes $out/.config/hypr/theme/colourme.lua, $out/.config/ghostty/themes/Luka,
# $out/.local/state/colourme/colours.json, ...
```

Preview:

```bash
colourme --dry-run Gruvbox
```

## Backwards compatibility

- No flags + one scheme name → identical behaviour to today (XDG default paths
  unless `XDG_CONFIG_HOME` differs from `~/.config`, which is the intended fix).
- `config.toml` schema is unchanged (`template`, `destination`, optional
  `post_hook`); relative `template` is newly meaningful.
- `~` expansion unchanged.

## Suggested implementation order

1. §1 parent dirs, §3 `--no-hooks`, §4 `--dest-root` (unblocks the plan).
2. §2 input paths (+ XDG), §7 relative resolution (portable config).
3. §5 atomic writes.
4. §6 CLI hygiene.

## Related, not required for the Nix plan

`REVIEW.md` bug fixes (B1 `blend`, B2 message, B3 `Config::new` panics) and
O1 (propagate hook failures), O4 (document symlink policy), O8 (scheme
validation). Worth folding in while in here, but independent of the above.
