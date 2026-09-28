//! Where things live: per-user cache/log dirs, and the Echo VR install layout.
//!
//! The Echo client always lives at `<root>/ready-at-dawn-echo-arena/bin/win10/echovr.exe`.
//! Install paths are kept in the Java form -- forward slashes, no trailing slash -- so the
//! saved `paths.properties` stays compatible with the old installer.

use std::path::{Path, PathBuf};

pub const ARENA_DIR: &str = "ready-at-dawn-echo-arena";
const ARENA_MARKER: &str = "ready-at-dawn-echo-arena/bin/win10/echovr.exe";

/// Per-user cache root. Never the shared temp dir (world-writable on Linux).
pub fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("EchoVR_Installer")
}

/// Staged downloads (Quest APK/data, licence patch dll, Revive installer).
pub fn downloads_dir() -> PathBuf {
    cache_dir().join("downloads")
}

/// Scratch space for short-lived files.
pub fn temp_dir() -> PathBuf {
    cache_dir().join("tmp")
}

/// Per-user application data (launcher state, logs).
pub fn data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("EchoVR_Installer")
}

pub fn log_dir() -> PathBuf {
    data_dir().join("logs")
}

/// Backslashes to slashes, trailing slashes trimmed.
pub fn normalize(path: &str) -> String {
    let mut n = path.trim().replace('\\', "/");
    while n.len() > 1 && n.ends_with('/') {
        n.pop();
    }
    n
}

pub fn bin_path(root: &str) -> PathBuf {
    PathBuf::from(format!("{root}/{ARENA_DIR}/bin/win10"))
}

pub fn exe_path(root: &str) -> PathBuf {
    bin_path(root).join("echovr.exe")
}

/// True when an Echo install exists directly under `root`.
pub fn has_echo_install(root: &str) -> bool {
    !root.is_empty() && Path::new(&format!("{root}/{ARENA_MARKER}")).is_file()
}

/// Resolves the Echo install ROOT from whatever folder the user picked: the root itself,
/// the `ready-at-dawn-echo-arena` folder, a folder inside it, or a folder up to three
/// levels above the install. Returns the cleaned selection unchanged when nothing is found
/// (the caller's validation then flags it invalid).
pub fn resolve_install_root(selected: &str) -> String {
    let norm = normalize(selected);
    if norm.is_empty() {
        return norm;
    }
    // 1) Walk up: the root is the ancestor (incl. the selection) holding the marker.
    let mut cur = Some(PathBuf::from(&norm));
    for _ in 0..8 {
        let Some(c) = cur else { break };
        let cs = normalize(&c.to_string_lossy());
        if has_echo_install(&cs) {
            return cs;
        }
        cur = c
            .parent()
            .map(Path::to_path_buf)
            .filter(|p| !p.as_os_str().is_empty());
    }
    // 2) Bounded downward search -- covers picking a folder above the install.
    search_down(Path::new(&norm), 3).unwrap_or(norm)
}

fn search_down(dir: &Path, depth: i32) -> Option<String> {
    if depth < 0 || !dir.is_dir() {
        return None;
    }
    let c = normalize(&dir.to_string_lossy());
    if has_echo_install(&c) {
        return Some(c);
    }
    let mut kids: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .collect();
    kids.sort();
    kids.iter().find_map(|k| search_down(k, depth - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_install(root: &Path) {
        let bin = root.join("ready-at-dawn-echo-arena/bin/win10");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("echovr.exe"), b"").unwrap();
    }

    #[test]
    fn normalizes() {
        assert_eq!(normalize("C:\\EchoVR\\"), "C:/EchoVR");
        assert_eq!(normalize("/"), "/");
        assert_eq!(normalize("  /a/b// "), "/a/b");
    }

    #[test]
    fn resolves_root_from_anywhere_nearby() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("games").join("Echo");
        fake_install(&root);
        let want = normalize(&root.to_string_lossy());
        for pick in [
            root.clone(),
            root.join("ready-at-dawn-echo-arena"),
            root.join("ready-at-dawn-echo-arena/bin/win10"),
            tmp.path().join("games"),
            tmp.path().to_path_buf(),
        ] {
            assert_eq!(
                resolve_install_root(&pick.to_string_lossy()),
                want,
                "{}",
                pick.display()
            );
        }
        assert!(has_echo_install(&want));
        let nothing = tmp.path().join("empty");
        std::fs::create_dir_all(&nothing).unwrap();
        let n = normalize(&nothing.to_string_lossy());
        assert_eq!(resolve_install_root(&n), n);
    }
}
