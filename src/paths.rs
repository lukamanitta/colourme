use std::path::{Path, PathBuf};

/// `$HOME`, erroring if it is unset or empty.
pub fn home_dir() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_string())
}

/// The current working directory, erroring if it cannot be determined.
pub fn current_dir() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|e| format!("Failed to determine current directory: {}", e))
}

/// `$XDG_CONFIG_HOME` when set and non-empty, otherwise `$HOME/.config`.
pub fn xdg_config_home(xdg: Option<&str>, home: &Path) -> PathBuf {
    match xdg {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => home.join(".config"),
    }
}

/// Expand a leading `~` or `~/` against `home`. Other paths are unchanged.
pub fn expand_tilde(input: &str, home: &Path) -> PathBuf {
    if input == "~" {
        home.to_path_buf()
    } else if let Some(rest) = input.strip_prefix("~/") {
        home.join(rest)
    } else {
        PathBuf::from(input)
    }
}

fn expand_tilde_path(path: &Path, home: &Path) -> PathBuf {
    expand_tilde(&path.to_string_lossy(), home)
}

/// Resolve the config file: an explicit `--config`/env path (tilde-expanded),
/// else `$XDG_CONFIG_HOME/colourme/config.toml`.
pub fn resolve_config_path(explicit: Option<&Path>, home: &Path, xdg: &Path) -> PathBuf {
    match explicit {
        Some(path) => expand_tilde_path(path, home),
        None => xdg.join("colourme").join("config.toml"),
    }
}

/// Resolve the schemes directory: an explicit `--schemes-dir`/env path
/// (tilde-expanded), else `$XDG_CONFIG_HOME/colourme/schemes`.
pub fn resolve_schemes_dir(explicit: Option<&Path>, home: &Path, xdg: &Path) -> PathBuf {
    match explicit {
        Some(path) => expand_tilde_path(path, home),
        None => xdg.join("colourme").join("schemes"),
    }
}

/// Resolve a destination root given on the command line, relative to `cwd`
/// when it is not absolute.
pub fn resolve_dest_root(root: &Path, cwd: &Path) -> PathBuf {
    if root.is_absolute() {
        root.to_path_buf()
    } else {
        cwd.join(root)
    }
}

/// Resolve a template: `~` against `home`, absolute as-is, otherwise relative
/// to the directory containing the config file.
pub fn resolve_template(template: &str, config_dir: &Path, home: &Path) -> PathBuf {
    let expanded = expand_tilde(template, home);
    if expanded.is_absolute() {
        expanded
    } else {
        config_dir.join(expanded)
    }
}

/// Re-root a destination that lives under `home` to `root`. Destinations
/// outside `home` are left unchanged.
fn apply_dest_root(destination: &Path, home: &Path, root: &Path) -> PathBuf {
    if destination == home {
        root.to_path_buf()
    } else if let Ok(relative) = destination.strip_prefix(home) {
        root.join(relative)
    } else {
        destination.to_path_buf()
    }
}

/// Resolve a destination: `~` against `home`, absolute as-is, otherwise
/// relative to `dest_root` (if set) or `cwd`. Any resulting path under `home`
/// is re-rooted to `dest_root` when one is set.
pub fn resolve_destination(
    destination: &str,
    dest_root: Option<&Path>,
    home: &Path,
    cwd: &Path,
) -> PathBuf {
    let expanded = expand_tilde(destination, home);
    let resolved = if expanded.is_absolute() {
        expanded
    } else if let Some(root) = dest_root {
        root.join(&expanded)
    } else {
        cwd.join(&expanded)
    };

    match dest_root {
        Some(root) => apply_dest_root(&resolved, home, root),
        None => resolved,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home() -> PathBuf {
        PathBuf::from("/home/user")
    }

    #[test]
    fn xdg_prefers_non_empty_env() {
        assert_eq!(
            xdg_config_home(Some("/xdg"), &home()),
            PathBuf::from("/xdg")
        );
        assert_eq!(
            xdg_config_home(Some(""), &home()),
            PathBuf::from("/home/user/.config")
        );
        assert_eq!(
            xdg_config_home(None, &home()),
            PathBuf::from("/home/user/.config")
        );
    }

    #[test]
    fn tilde_expansion() {
        assert_eq!(expand_tilde("~", &home()), PathBuf::from("/home/user"));
        assert_eq!(
            expand_tilde("~/.config/x", &home()),
            PathBuf::from("/home/user/.config/x")
        );
        assert_eq!(expand_tilde("/etc/x", &home()), PathBuf::from("/etc/x"));
        assert_eq!(expand_tilde("rel/x", &home()), PathBuf::from("rel/x"));
    }

    #[test]
    fn default_paths_use_xdg() {
        let xdg = PathBuf::from("/home/user/.config");
        assert_eq!(
            resolve_config_path(None, &home(), &xdg),
            PathBuf::from("/home/user/.config/colourme/config.toml")
        );
        assert_eq!(
            resolve_schemes_dir(None, &home(), &xdg),
            PathBuf::from("/home/user/.config/colourme/schemes")
        );
    }

    #[test]
    fn explicit_paths_override_and_expand_tilde() {
        let xdg = PathBuf::from("/xdg");
        assert_eq!(
            resolve_config_path(Some(Path::new("/tmp/c.toml")), &home(), &xdg),
            PathBuf::from("/tmp/c.toml")
        );
        assert_eq!(
            resolve_config_path(Some(Path::new("~/c.toml")), &home(), &xdg),
            PathBuf::from("/home/user/c.toml")
        );
        assert_eq!(
            resolve_schemes_dir(Some(Path::new("~/s")), &home(), &xdg),
            PathBuf::from("/home/user/s")
        );
    }

    #[test]
    fn templates_resolve_against_config_dir() {
        let config_dir = PathBuf::from("/tmp/cfg");
        assert_eq!(
            resolve_template("t/hypr.lua", &config_dir, &home()),
            PathBuf::from("/tmp/cfg/t/hypr.lua")
        );
        assert_eq!(
            resolve_template("/abs/t.lua", &config_dir, &home()),
            PathBuf::from("/abs/t.lua")
        );
        assert_eq!(
            resolve_template("~/t.lua", &config_dir, &home()),
            PathBuf::from("/home/user/t.lua")
        );
    }

    #[test]
    fn destinations_without_dest_root() {
        let cwd = PathBuf::from("/tmp/work");
        assert_eq!(
            resolve_destination("~/.config/x", None, &home(), &cwd),
            PathBuf::from("/home/user/.config/x")
        );
        assert_eq!(
            resolve_destination("/etc/x", None, &home(), &cwd),
            PathBuf::from("/etc/x")
        );
        assert_eq!(
            resolve_destination("rel/x", None, &home(), &cwd),
            PathBuf::from("/tmp/work/rel/x")
        );
    }

    #[test]
    fn destinations_re_root_under_home() {
        let cwd = PathBuf::from("/tmp/work");
        let root = Path::new("/out");
        assert_eq!(
            resolve_destination("~/.config/x", Some(root), &home(), &cwd),
            PathBuf::from("/out/.config/x")
        );
        assert_eq!(
            resolve_destination("~", Some(root), &home(), &cwd),
            PathBuf::from("/out")
        );
        assert_eq!(
            resolve_destination("/etc/x", Some(root), &home(), &cwd),
            PathBuf::from("/etc/x")
        );
        assert_eq!(
            resolve_destination("/home/user2/x", Some(root), &home(), &cwd),
            PathBuf::from("/home/user2/x")
        );
    }

    #[test]
    fn relative_destinations_use_dest_root() {
        let cwd = PathBuf::from("/tmp/work");
        let root = Path::new("/out");
        assert_eq!(
            resolve_destination("rel/x", Some(root), &home(), &cwd),
            PathBuf::from("/out/rel/x")
        );
    }

    #[test]
    fn relative_dest_root_resolves_against_cwd() {
        let cwd = PathBuf::from("/tmp/work");
        assert_eq!(
            resolve_dest_root(Path::new("root"), &cwd),
            PathBuf::from("/tmp/work/root")
        );
        assert_eq!(
            resolve_dest_root(Path::new("/root"), &cwd),
            PathBuf::from("/root")
        );
    }
}
