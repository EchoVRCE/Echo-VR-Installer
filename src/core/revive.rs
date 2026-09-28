//! SteamVR support through Revive: locating/installing Revive,
//! the Revive-injector desktop shortcut, and the Meta Horizon store artwork.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::{bail, Context, Result};

pub const REVIVE_INSTALLER_URL: &str =
    "https://github.com/LibreVR/Revive/releases/download/3.1.1/ReviveInstaller.exe";
/// Pinned: the installer runs elevated, so only this exact build is ever executed.
pub const REVIVE_INSTALLER_SHA256: &str =
    "f4832a6193d55c2477c8da19179457c4f777600f48c67e670015103d7189c091";
pub const DEFAULT_REVIVE_DIR: &str = "C:\\Program Files\\Revive";
pub const REVIVE_INJECTOR: &str = "ReviveInjector.exe";
pub const APP_ID: &str = "ready-at-dawn-echo-arena";
#[allow(dead_code)] // for the launcher's revive.vrmanifest support
pub const APP_KEY: &str = "revive.app.ready-at-dawn-echo-arena";
pub const STORE_ASSETS_DIR: &str =
    "C:\\Program Files\\Meta Horizon\\CoreData\\Software\\StoreAssets\\ready-at-dawn-echo-arena_assets";
#[allow(dead_code)] // for the launcher's revive.vrmanifest support
pub const IMAGE_PATH: &str = "C:/Program Files/Meta Horizon/CoreData/Software/StoreAssets/ready-at-dawn-echo-arena_assets/cover_landscape_image_large.png";
pub const ARTWORK_ZIP_URL: &str =
    "https://files.echovr.de/stuff/patches/ready-at-dawn-echo-arena_assets.zip";
pub const SHORTCUT_NAME: &str = "Echo VR (Revive)";

/// The directory if it contains ReviveInjector.exe, trailing separators trimmed.
fn verify_revive_dir(dir: &str) -> Option<String> {
    let trimmed = dir.trim().trim_end_matches(['\\', '/']);
    if trimmed.is_empty() {
        return None;
    }
    Path::new(trimmed)
        .join(REVIVE_INJECTOR)
        .is_file()
        .then(|| trimmed.to_string())
}

/// Locates an installed Revive: the uninstall registry's InstallLocation first, then the
/// default folder. The candidate is always verified, since the registry can be stale.
pub fn find_revive_dir() -> Option<String> {
    if !cfg!(windows) {
        return None;
    }
    if let Some(dir) = super::platform::revive_install_location() {
        tracing::info!("Revive: registry InstallLocation = '{dir}'");
        if let Some(v) = verify_revive_dir(&dir) {
            return Some(v);
        }
    }
    verify_revive_dir(DEFAULT_REVIVE_DIR)
}

/// Polls [`find_revive_dir`] until Revive shows up or the timeout elapses.
pub fn wait_for_revive_dir(timeout: std::time::Duration) -> Option<String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(d) = find_revive_dir() {
            return Some(d);
        }
        if std::time::Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Downloads the pinned Revive installer into the per-user download dir and verifies it.
pub fn download_installer(
    cancel: &AtomicBool,
    on: &mut dyn FnMut(super::download::Progress),
) -> Result<PathBuf> {
    let job = super::download::Job {
        url: REVIVE_INSTALLER_URL.into(),
        dir: super::paths::downloads_dir().join("revive"),
        filename: "ReviveInstaller.exe".into(),
        use_mirror: false,
        fresh: false,
        extract: false,
    };
    let path = super::download::run(&job, cancel, on)?;
    if !super::download::sha256_matches(&path, REVIVE_INSTALLER_SHA256) {
        let _ = std::fs::remove_file(&path);
        bail!("The downloaded Revive installer failed its integrity check. Please try again.");
    }
    Ok(path)
}

/// Arguments of the injector shortcut -- the "can't press any buttons in-game" fix.
pub fn injector_arguments(exe: &str) -> String {
    format!("\"{exe}\" -nosymbollookup /app {APP_ID}")
}

/// Creates the desktop shortcut launching Echo VR through the Revive injector.
pub fn create_injector_shortcut(revive_dir: &str, exe: &Path) -> Result<()> {
    let injector = Path::new(revive_dir).join(REVIVE_INJECTOR);
    let exe_abs = std::path::absolute(exe).unwrap_or_else(|_| exe.to_path_buf());
    let exe_str = exe_abs.to_string_lossy().replace('/', "\\");
    super::platform::create_shortcut(
        SHORTCUT_NAME,
        &injector,
        Some(&injector_arguments(&exe_str)),
        Some(Path::new(revive_dir)),
        Some(&injector),
    )
}

/// Downloads the game artwork and extracts it into the Meta Horizon store assets.
pub fn install_artwork(cancel: &AtomicBool) -> Result<()> {
    let dir = super::paths::downloads_dir().join("revive");
    std::fs::create_dir_all(&dir)?;
    let zip = dir.join(format!("{APP_ID}_assets.zip"));
    super::http::download_to(ARTWORK_ZIP_URL, &zip, Some(cancel))
        .context("Downloading the artwork failed")?;
    let dest = Path::new(STORE_ASSETS_DIR);
    std::fs::create_dir_all(dest).with_context(|| format!("create {STORE_ASSETS_DIR}"))?;
    super::zip::extract(&zip, dest, cancel)?;
    let _ = std::fs::remove_file(&zip);
    Ok(())
}

/// True for errors that elevation would fix.
pub fn needs_elevation(e: &anyhow::Error) -> bool {
    e.chain().any(|c| {
        c.downcast_ref::<std::io::Error>().is_some_and(|io| {
            io.kind() == std::io::ErrorKind::PermissionDenied
                || io.raw_os_error() == Some(5)   // ERROR_ACCESS_DENIED
                || io.raw_os_error() == Some(740) // ERROR_ELEVATION_REQUIRED
        })
    })
}

// ---- revive.vrmanifest (not wired in yet) ----

/// Extracts the shared library id from the first existing entry's `/library <id>`.
#[allow(dead_code)] // for the launcher's revive.vrmanifest support
pub fn detect_library_id(apps: &[serde_json::Value]) -> Option<String> {
    apps.iter()
        .filter_map(|a| a.get("arguments")?.as_str())
        .find_map(|args| {
            let mut it = args.split_whitespace();
            while let Some(tok) = it.next() {
                if tok == "/library" {
                    let id = it.next()?;
                    if !id.eq_ignore_ascii_case("put-library-ID-here") {
                        return Some(id.to_string());
                    }
                }
            }
            None
        })
}

#[allow(dead_code)] // for the launcher's revive.vrmanifest support
pub fn echo_manifest_entry(library_id: &str) -> serde_json::Value {
    serde_json::json!({
        "action_manifest_path": "Input/action_manifest.json",
        "app_key": APP_KEY,
        "arguments": format!("/app {APP_ID} /library {library_id} \"Software\\ready-at-dawn-echo-arena\\bin\\win10\\echovr.exe\" -nosymbollookup"),
        "binary_path_windows": REVIVE_INJECTOR,
        "image_path": IMAGE_PATH,
        "launch_type": "binary",
        "strings": { "en_us": { "name": APP_ID } }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_id_detection() {
        let apps = vec![
            serde_json::json!({"app_key": "x"}),
            serde_json::json!({"arguments": "/app a /library put-library-ID-here x"}),
            serde_json::json!({"arguments": "/app b /library Software2 \"x.exe\""}),
        ];
        assert_eq!(detect_library_id(&apps).as_deref(), Some("Software2"));
        assert_eq!(detect_library_id(&[]), None);
    }

    #[test]
    fn manifest_entry_shape() {
        let e = echo_manifest_entry("Lib");
        assert_eq!(e["app_key"], APP_KEY);
        assert!(e["arguments"].as_str().unwrap().contains("/library Lib"));
        assert_eq!(e["strings"]["en_us"]["name"], APP_ID);
    }

    #[test]
    fn injector_args() {
        assert_eq!(
            injector_arguments("C:\\EchoVR\\ready-at-dawn-echo-arena\\bin\\win10\\echovr.exe"),
            "\"C:\\EchoVR\\ready-at-dawn-echo-arena\\bin\\win10\\echovr.exe\" -nosymbollookup /app ready-at-dawn-echo-arena"
        );
    }

    #[test]
    fn elevation_detection() {
        let e = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
            .context("x");
        assert!(needs_elevation(&e));
        assert!(!needs_elevation(&anyhow::anyhow!("network")));
    }
}
