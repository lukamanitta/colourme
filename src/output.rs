use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Resolve a destination's symlink chain to the real path it points at.
///
/// `Path::canonicalize` cannot be used here because it fails for dangling
/// symlinks and for paths whose ancestors do not exist yet. Following links
/// manually lets us keep the "write through symlinks" behaviour (a dotfiles
/// file linked into place) while still handling dangling links and missing
/// parents. A path that does not exist resolves to itself.
fn resolve_symlink_target(path: &Path) -> Result<PathBuf, String> {
    let mut current = path.to_path_buf();
    let mut seen: Vec<PathBuf> = Vec::new();

    loop {
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                if seen.contains(&current) {
                    return Err(format!(
                        "Refusing to write {}: symlink chain loops",
                        path.display()
                    ));
                }
                seen.push(current.clone());
                let link = fs::read_link(&current)
                    .map_err(|e| format!("Failed to read symlink {}: {}", current.display(), e))?;
                current = if link.is_absolute() {
                    link
                } else {
                    current
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(link)
                };
            }
            _ => return Ok(current),
        }
    }
}

fn temp_path(directory: &Path) -> PathBuf {
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    directory.join(format!(".colourme.{}.{}.tmp", std::process::id(), counter))
}

/// Write `content` to `destination`, atomically and following symlinks.
///
/// The destination's symlink chain is resolved first so links are written
/// through rather than replaced. Missing parent directories are created, the
/// content is written to a temporary file in the target directory (guaranteeing
/// a same-filesystem rename), fsynced, and then renamed over the target. No
/// partially written file is ever visible, and the temp file is removed if any
/// step fails.
pub fn write_atomically(destination: &Path, content: &str) -> Result<(), String> {
    let target = resolve_symlink_target(destination)?;

    if let Ok(meta) = fs::symlink_metadata(&target) {
        if meta.is_dir() {
            return Err(format!("Destination {} is a directory", target.display()));
        }
    }

    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf);

    if let Some(parent) = parent.as_deref() {
        if let Ok(meta) = fs::symlink_metadata(parent) {
            if !meta.is_dir() {
                return Err(format!(
                    "Cannot write {}: parent {} is not a directory",
                    target.display(),
                    parent.display()
                ));
            }
        }
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    let directory = parent.unwrap_or_else(|| PathBuf::from("."));
    let temp = temp_path(&directory);

    let write_result = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&temp)?;
        file.write_all(content.as_bytes())?;
        // fsync so a crash after rename cannot leave a shorter/empty file.
        file.sync_all()?;
        Ok(())
    })();

    if let Err(e) = write_result {
        let _ = fs::remove_file(&temp);
        return Err(format!("Failed to write {}: {}", target.display(), e));
    }

    if let Err(e) = fs::rename(&temp, &target) {
        let _ = fs::remove_file(&temp);
        return Err(format!(
            "Failed to move output into place at {}: {}",
            target.display(),
            e
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let counter = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "colourme_output_test_{}_{}",
                std::process::id(),
                counter
            ));
            fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    #[test]
    fn creates_missing_parent_directories() {
        let dir = TempDir::new();
        let destination = dir.path().join("a/b/c/out.txt");

        write_atomically(&destination, "hello").unwrap();

        assert_eq!(fs::read_to_string(&destination).unwrap(), "hello");
    }

    #[test]
    #[cfg(unix)]
    fn writes_through_symlink_and_keeps_it() {
        let dir = TempDir::new();
        let target = dir.path().join("target.txt");
        fs::write(&target, "old").unwrap();
        let link = dir.path().join("link.txt");
        symlink(&target, &link).unwrap();

        write_atomically(&link, "new").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn dangling_symlink_creates_target() {
        let dir = TempDir::new();
        let target = dir.path().join("missing/target.txt");
        let link = dir.path().join("link.txt");
        symlink(&target, &link).unwrap();

        write_atomically(&link, "created").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "created");
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    #[cfg(unix)]
    fn failed_write_leaves_old_content_and_no_temp_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new();
        let destination = dir.path().join("out.txt");
        fs::write(&destination, "original").unwrap();

        let read_only = dir.path().join("readonly");
        fs::create_dir(&read_only).unwrap();
        fs::set_permissions(&read_only, fs::Permissions::from_mode(0o555)).unwrap();
        let locked_destination = read_only.join("out.txt");

        let result = write_atomically(&locked_destination, "nope");

        // Restore permissions so the TempDir can be cleaned up.
        fs::set_permissions(&read_only, fs::Permissions::from_mode(0o755)).unwrap();

        assert!(
            result.is_err(),
            "expected writing into a read-only dir to fail"
        );
        assert!(!locked_destination.exists());
        assert_eq!(fs::read_to_string(&destination).unwrap(), "original");
        let leftovers: Vec<_> = fs::read_dir(&read_only)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp file left behind: {:?}",
            leftovers
        );
    }

    #[test]
    fn directory_destination_errors() {
        let dir = TempDir::new();
        let sub = dir.path().join("adir");
        fs::create_dir(&sub).unwrap();

        let err = write_atomically(&sub, "x").unwrap_err();
        assert!(err.contains("is a directory"), "got: {}", err);
    }
}
