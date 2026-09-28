//! "Delete cache": removes downloaded/staged files.
//!
//! The Java version only knew two hard-coded zip locations and told the user to delete
//! the rest by hand; this also removes the game zip at the saved install path, and stops
//! the adb server first so its binary is not locked on Windows.

use std::path::{Path, PathBuf};

fn targets() -> Vec<PathBuf> {
    let mut v = vec![super::paths::cache_dir()];
    let zip = format!("{}.zip", super::paths::ARENA_DIR);
    let mut roots = vec![
        "C:/EchoVR".to_string(),
        "C:/Program Files/Oculus/Software/Software".to_string(),
    ];
    if let Some(saved) = super::config::load_install_path() {
        roots.push(saved);
    }
    for r in roots {
        v.push(Path::new(&r).join(&zip));
    }
    // Leftovers of the Java installer in the shared temp dir.
    let tmp = std::env::temp_dir();
    for d in [
        "echo",
        "revive",
        "platform-tools",
        "platform-tools-linux",
        "platform-tools-mac",
    ] {
        v.push(tmp.join(d));
    }
    v
}

/// Deletes everything it can; returns the paths that existed but could not be removed.
pub fn delete_all() -> Vec<PathBuf> {
    super::adb::exec(&["kill-server"]);
    let mut failed = Vec::new();
    for p in targets() {
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
