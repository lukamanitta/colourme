# Review notes

Findings from reading through the source while planning the NixOS integration.
Not a code review of style — just bugs and oversights worth fixing. Line numbers
are against HEAD `71ab33c`.

## Bugs

### B1 — `blend` ignores its second colour
`src/parser/functions.rs:111`

`builtin_blend` builds `colour2` from `args[0]`, not `args[1]`:

```rust
let mut constructed_colour2: Option<Colour>;
let colour2 = match &args[0] {   // <-- should be args[1]
```

So `blend(a, b, r)` always blends `a` with itself, and `b` is silently dropped.
The error string on the following lines even says "Second argument" while
inspecting `args[0]`.

### B2 — wrong error message in `h`
`src/parser/functions.rs:17`

`builtin_h` reports `"First argument to darken must be a colour"` (copy-pasted
from `darken`); should mention `h`.

### B3 — `Config::new` panics instead of returning an error
`src/config.rs:24,29,30`

```rust
let config_table = toml_string.parse::<Table>().unwrap();          // 24
template_path: shellexpand::tilde(&value["template"].as_str().unwrap()) // 29
destination_path: shellexpand::tilde(&value["destination"].as_str().unwrap()) // 30
```

An invalid `config.toml`, a missing `template`/`destination`, or a non-string
value panics (indexing a missing key panics, and `.unwrap()` on `as_str()`).
`Config::new` returns `Config`, not `Result`, so the caller can't recover. A
typo in one entry takes down the whole run with a Rust backtrace rather than a
message naming the offending entry.

## Oversights / robustness

### O1 — post-hook failures are swallowed
`src/main.rs:181-217`

`run_post_hook` prints to stderr on non-zero exit or spawn failure but still
returns `Ok(())`, so `colourme` exits 0 even when every reload hook failed. A
theme switch can appear to succeed while the desktop never reloaded. Consider
propagating failure (or at least a distinct exit code / summary).

### O2 — output is not written atomically
`src/main.rs:165-179`

`write_output` truncates the destination and then writes. If the process dies
mid-write (or the disk fills), the destination is left truncated/corrupt. Since
destinations are live config files read by running programs, a temp-file +
`rename` would be safer.

### O3 — parent directories are not created
`src/main.rs:165-171`

`OpenOptions::open` fails if the destination's parent dir doesn't exist (no
`create_dir_all`). On a fresh machine the first run fails for every target whose
directory isn't already present (e.g. `~/.config/hypr/theme/`,
`~/.config/ghostty/themes/`).

### O4 — behaviour on symlinked destinations is implicit
`src/main.rs:165-179`

Because it opens the path directly, `colourme` writes *through* symlinks. That's
useful (targeting a dotfiles file), but a symlink pointing into a read-only
location (e.g. a Nix store path) fails at `open` with a confusing error. Worth
deciding/documenting: follow (current) vs replace (`rename`), and detecting the
read-only case.

### O5 — hardcoded, non-overridable paths; ignores `XDG_CONFIG_HOME`
`src/main.rs:68-78`

Config is always `~/.config/colourme/config.toml` and schemes always
`~/.config/colourme/schemes/<name>.toml`. No CLI flag or env var overrides
these, and `XDG_CONFIG_HOME` is ignored. Also, `shellexpand::tilde` only expands
a leading `~`; `${VAR}`/`$VAR` in `template`/`destination` are not expanded.

### O6 — CLI has no flags
`src/main.rs:262-286`

Exactly one positional arg is accepted; `--help`, `-h`, `--version` are treated
as scheme names (and fail trying to open `~/.config/colourme/schemes/--help.toml`).
There's no way to list available schemes, do a dry run, or point at a different
config/scheme.

### O7 — `lighten` / `invert` / `invert_brightness` are not registered
`src/parser/evaluator.rs:27-32`, `src/parser/functions.rs:1-4`

`colour_utils::operations` exposes `lighten`, `invert`, and `invert_brightness`,
but none is imported or registered, so any template using `lighten(...)` fails
with "Undefined function: lighten". (`parser.rs` tests already reference
`lighten(darken(...))`.)

### O8 — schemes are untyped; missing keys fail late
`src/main.rs:80-86`, `src/parser/evaluator.rs:135-157`

A scheme is a free-form `toml::Table`; there is no schema or required-key check.
Templates paper over missing keys with `||` fallbacks (e.g. `Catppuccin.toml` has
no `accent_alt_1`/`accent_alt_2`). A genuinely absent key only surfaces as a
render-time error; there's no warning that, say, `base16` is missing.

### O9 — `random_select` reuse is label-string based
`src/main.rs:94-128,223-224`

Resolved expressions are cached by their exact literal text across all
templates in a run, so identical `random_select(...)` expressions reuse the
first result (the intent behind TODO "repeatable random_select"). The flip side
is that whitespace/argument-order changes silently produce a different draw.
Worth making the caching rule explicit (e.g. key on the resolved expression, or
an explicit `?random` marker).

### O10 — "multiple templates per entry" still pending
`src/config.rs:6-12`, `TODO.md:7`

Each config entry maps exactly one `template` to one `destination`. Common
groups (e.g. several Gui/terminal files sharing a palette) currently need one
entry each. Already tracked in `TODO.md`.

### O11 — no docs/test-suite entry point
`TODO.md:3-4`

Not a bug, but there's no README or usage doc, and `TODO.md` already flags the
workflow as hard to test. Worth folding the above into that pass.
