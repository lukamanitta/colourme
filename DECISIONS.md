# Decisions log

Judgement calls made while implementing `INTERFACE_CHANGES.md` and the
findings in `REVIEW.md`, recorded for review. Each links the change to the
commit series.

## D1 — Use `clap` (derive) for the CLI

`INTERFACE_CHANGES.md` §6 allows clap if adding a dependency is
acceptable. It gives correct `--help`/`--version`/unknown-flag handling and
exit-2 usage errors for free, and flag/env precedence via the `env`
attribute. Implemented with `features = ["derive", "env"]`.

## D2 — No `tempfile` dependency; temp name from pid + counter; fsync

§5 suggests `.colourme.<pid>.<rand>.tmp`. The existing `rand` dependency
could be used, but a process-local `AtomicU64` counter plus the pid is
already collision-free for our single-threaded writer and easier to test.
We `fsync` (`File::sync_all`) the temp file before rename; the interface
marks this optional, but it closes the "crash after rename leaves a short
file" window at negligible cost.

## D3 — `$VAR` expansion intentionally omitted (addresses O5 only in part)

REVIEW O5 notes that `shellexpand::tilde` never expanded `${VAR}`/`$VAR`.
We fixed the more important half (XDG-aware, overridable paths) and
deliberately did **not** add environment expansion: the Nix goal is a
*portable* config, and ambient-env expansion would make rendering depend
on the caller's environment and could corrupt paths containing `$`. The
portable case is served by relative paths (templates relative to the
config file; destinations relative to `--dest-root`). Documented in the
README. Easy to revisit if a concrete need appears.

## D4 — `list` is a reserved positional keyword, not a subcommand

The spec's target CLI is `colourme [OPTIONS] <scheme>` and
`colourme [OPTIONS] list`. Treating `list` as a positional keyword keeps a
single parser shape and keeps all options available in both forms. The
trade-off is that a scheme literally named `list` cannot be rendered;
`list` is documented as reserved.

## D5 — O1: aggregate hook failures, keep going, exit 1

The review offered "propagate failure (or at least a distinct exit code /
summary)". Rather than abort at the first failing hook (which would leave
later entries unwritten and unordered relative to a reload), we still run
every entry, collect each failure, and return one aggregated error so the
process exits 1. None of the hooks run under `--dry-run` or `--no-hooks`.

## D6 — O8: warn only for blocks with no `||` fallback

A missing key that is masked by a fallback is an explicit authoring
choice (e.g. Catppuccin lacking `accent_alt_1`), so warning there would be
noise. We warn only when a `{{ ... }}` block has a single expression that
references an absent scheme path, deduplicated per entry. This surfaces a
genuinely missing key before the render-time error without changing
fallback behaviour. The scan is best-effort: unparseable templates are
left for the renderer to report.

## D7 — O9: caching rule documented, semantics unchanged

Resolved expressions are cached by their exact literal text across the
run, which is what makes `random_select` repeatable. Whitespace/argument
changes produce a different key and therefore an independent draw. We
kept this rule and documented it in `main.rs` rather than re-keying on the
resolved expression, which would not change repeatability and would add
complexity.

## D8 — O10 (multiple templates per entry) left to `TODO.md`

As the review notes, this is already tracked and is a config-schema
change beyond the Nix seam. Not addressed here to keep the config schema
backwards compatible.

## D9 — `invert_brightness` fixed in `colour_utils`

Registering `invert_brightness` (O7) exposed a latent bug: HSV value spans
0–100 but the operation computed `1.0 - v`, producing out-of-range values.
Fixed there (`2a48fce`) with tests for `invert` and `invert_brightness`
and committed separately, as requested.

## D10 — Config stores raw paths; resolution lives in `paths`

`Config` no longer expands `~`; it stores the declared strings. All
resolution (tilde, XDG, relative-to-config, `--dest-root` re-rooting) is
centralised in the pure, unit-tested `paths` module. This is what makes
the portable-config behaviour testable without touching the environment.

## D11 — `--dest-root` applies only to `$HOME`-based destinations

Per §4, destinations outside `$HOME` are left untouched (rather than
erroring). Re-rooting uses `Path::strip_prefix`, so `~` alone maps to the
root and `~/x` maps to `root/x`. Relative roots resolve against the CWD at
CLI-parse time.
