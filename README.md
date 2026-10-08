# colourme

Render a colour scheme into the config files declared in a `config.toml`.
Useful for keeping a themed desktop (hyprland, ghostty, etc.) in sync when
switching between schemes.

## Usage

```
colourme [OPTIONS] <scheme>
colourme [OPTIONS] list
```

| Flag | Argument | Meaning |
|---|---|---|
| `--config` | `PATH` | Config TOML to use (default: `$XDG_CONFIG_HOME/colourme/config.toml`) |
| `--schemes-dir` | `DIR` | Directory of `<scheme>.toml` files (default: `$XDG_CONFIG_HOME/colourme/schemes`) |
| `--dest-root` | `DIR` | Re-root `$HOME`-based destinations under `DIR` |
| `--no-hooks` | — | Render + write, but run no `post_hook` commands |
| `--dry-run` | — | Resolve everything, run nothing, write nothing; print the plan |
| `-h`, `--help` | — | Usage, exit 0 |
| `-V`, `--version` | — | Version, exit 0 |

Environment variables (lower precedence than flags): `COLOURME_CONFIG`,
`COLOURME_SCHEMES_DIR`. Input paths resolve as **flag > env > XDG
default**, where the XDG base is `$XDG_CONFIG_HOME` if set and non-empty,
otherwise `$HOME/.config`.

Exit codes: `0` success, `1` runtime error (I/O, parse, render, failed
hooks), `2` usage error (bad flag or arity).

### Examples

Live apply:

```bash
colourme Gruvbox
```

Preview:

```bash
colourme --dry-run Gruvbox
```

Hermetic build-time render (e.g. from a Nix derivation):

```bash
colourme \
  --config "$src/config.toml" \
  --schemes-dir "$src/schemes" \
  --dest-root "$out" \
  --no-hooks \
  Gruvbox
```

## Config format

```toml
[hypr]
template = "templates/hypr.lua"          # written into each file
destination = "~/.config/hypr/theme/colourme.lua"
post_hook = "hyprctl reload"             # optional

[ghostty]
template = "templates/ghostty"
destination = "~/.config/ghostty/themes/Luka"
```

- `template` and `destination` are required strings; `post_hook` is
  optional. A malformed config names the offending entry and exits 1.
- A **relative** `template` resolves against the directory containing the
  config file, so a config plus its `templates/` directory is portable
  between `~/.config/colourme` and a store path.
- A **relative** `destination` resolves against `--dest-root` if set,
  otherwise the current working directory.
- A leading `~` expands to `$HOME`; absolute paths are used as-is.
  Shell-style `$VAR` expansion is intentionally *not* performed (see
  `DECISIONS.md`).

### `--dest-root`

Destinations under `$HOME` (whether written as `~`, `~/...` or an
absolute path beneath it) are re-rooted under the given directory, so one
config can generate a self-contained tree. Destinations outside `$HOME`
are left unchanged; a config that targets such an absolute path will fail
at write time when rendered into a sandbox.

## Schemes and templates

A scheme is a TOML file of named values:

```toml
[colors]
primary = "#FF0000"
secondary = "#00FF00"
```

Templates use `{{ <format>:<expression> }}` blocks. Formats include
`hex`, `hexa`, `rgb`, `rgba`, `hsv`, `str` and `num`. Expressions may
reference scheme keys, literals, and functions such as `darken`,
`lighten`, `blend`, `invert`, `invert_brightness`, `h`, `hsv`,
`multiply_brightness` and `random_select`, and may provide fallbacks with
`||`:

```
background = {{hex:colors.base || hex:#1e1e2e}}
accent     = {{hex:darken(colors.primary, 2.0)}}
```

If a `{{ ... }}` block with no fallback references a key the scheme lacks,
`colourme` prints a warning before rendering (see `DECISIONS.md`).

## Writes

Output is written atomically: a temporary file is created in the target
directory, fsynced, and renamed over the destination, so a crash or full
disk never leaves a partially written config. Missing parent directories
are created. Symlinks are followed (write-through), so linking a dotfiles
file into place keeps working.

## Development

```bash
cargo test
cargo run -- --dry-run Gruvbox
```

See `DECISIONS.md` for judgement calls made while implementing the
interface changes described in `INTERFACE_CHANGES.md` and the findings in
`REVIEW.md`.
