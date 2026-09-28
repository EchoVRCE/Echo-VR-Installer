//! "Delete cache": removes downloaded/staged files, and the game zips that cancelled
//! installs leave in the launcher's version folders. Stops the adb server first so its
//! binary is not locked on Windows.

use std::path::{Path, PathBuf};

fn targets(roots: &[String]) -> Vec<PathBuf> {
    let zip = format!("{}.zip", super::paths::ARENA_DIR);
    let mut v = vec![super::paths::cache_dir()];
    v.extend(roots.iter().map(|r| Path::new(r).join(&zip)));
    v
}

/// Deletes everything it can; returns the paths that existed but could not be removed.
/// `roots` are the version folders the launcher installs into.
pub fn delete_all(roots: &[String]) -> Vec<PathBuf> {
    super::adb::exec(&["kill-server"]);
    let mut failed = Vec::new();
    for p in targets(roots) {
        let result = if p.is_dir() {
            std::fs::remove_dir_all(&p)
        } else if p.exists() {
            std::fs::remove_file(&p)
        } else {
            continue;
        };
        match result {
            Ok(()) => tracing::info!("Deleted: {}", p.display()),
            Err(e) => {
                tracing::warn!("Failed to delete {}: {e}", p.display());
                failed.push(p);
            }
        }
    }
    failed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_our_folders() {
        let t = targets(&["C:/EchoVR/versions/pc-34.4".into()]);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0], crate::core::paths::cache_dir());
        assert!(t[1].ends_with("pc-34.4/ready-at-dawn-echo-arena.zip"));
    }
}
