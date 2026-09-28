//! Installing, updating, verifying and removing one PC version in the library.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::{bail, Context, Result};

use super::catalog::{Platform, VersionEntry};
use super::store::InstalledVersion;
use crate::core::download::{self, Progress};
use crate::core::manifest::Manifest;
use crate::core::{paths, pc_update};

const ZIP_NAME: &str = "ready-at-dawn-echo-arena.zip";

/// Progress of a long-running version job, for the UI row.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Status(String),
    Percent(f64),
}

pub fn root_for(library: &str, id: &str) -> String {
    paths::normalize(&format!("{}/{id}", paths::normalize(library)))
}

/// Downloads, verifies and extracts `entry` into `<library>/<id>`, then applies its update
/// manifest. The multi-GB zip is deleted afterwards (the launcher keeps no cache copy).
pub fn install(
    entry: &VersionEntry,
    library: &str,
    cancel: &AtomicBool,
    on: &mut dyn FnMut(Step),
) -> Result<InstalledVersion> {
    if entry.platform != Platform::Pc {
        bail!("Quest versions are installed with the Quest wizard.");
    }
    let root = root_for(library, &entry.id);
    if paths::has_echo_install(&root) {
        bail!("{} is already installed at {root}.", entry.name);
    }
    let job = download::Job {
        url: entry.url.clone(),
        dir: PathBuf::from(&root),
        filename: ZIP_NAME.into(),
        use_mirror: entry.uses_mirror(),
        fresh: false,
        extract: false,
    };
    let zip = download::run(&job, cancel, &mut |p| {
        on(match p {
            Progress::Status(s) => Step::Status(s),
            Progress::Percent(v) => Step::Percent(v),
            Progress::Extracting => Step::Status("Extracting...".into()),
            Progress::Extracted => Step::Status("Extraction complete".into()),
        })
    })?;
    if let Some(sha) = &entry.sha256 {
        on(Step::Status("Verifying download...".into()));
        if !download::sha256_matches(&zip, sha) {
            let _ = std::fs::remove_file(&zip);
            bail!("The downloaded game files are corrupt (checksum mismatch). Please try again.");
        }
    }
    on(Step::Status("Extracting...".into()));
    crate::core::zip::extract(&zip, Path::new(&root), cancel)?;
    let _ = std::fs::remove_file(&zip);

    if let Some(m) = &entry.update_manifest {
        on(Step::Status("Applying update...".into()));
        pc_update::apply(m, &paths::bin_path(&root), cancel, &mut |s| {
            on(Step::Status(s))
        })?;
    }
    if !paths::has_echo_install(&root) {
        bail!("The download did not contain Echo VR (echovr.exe is missing).");
    }
    Ok(InstalledVersion {
        id: entry.id.clone(),
        name: entry.name.clone(),
        root,
        external: false,
        catalog_id: Some(entry.id.clone()),
        update_manifest: entry.update_manifest.clone(),
        installed_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .ok(),
    })
}

fn manifest_url(v: &InstalledVersion) -> &str {
    v.update_manifest
        .as_deref()
        .unwrap_or(pc_update::PC_MANIFEST_URL)
}

pub fn update(v: &InstalledVersion, cancel: &AtomicBool, on: &mut dyn FnMut(Step)) -> Result<()> {
    ensure_present(v)?;
    pc_update::apply(
        manifest_url(v),
        &paths::bin_path(&v.root),
        cancel,
        &mut |s| on(Step::Status(s)),
    )
}

/// Files of the update manifest that are missing or differ on disk.
pub fn verify(v: &InstalledVersion, on: &mut dyn FnMut(Step)) -> Result<Vec<String>> {
    ensure_present(v)?;
    let m = Manifest::fetch(manifest_url(v))?;
    let bin = paths::bin_path(&v.root);
    let adds: Vec<_> = m.adds().collect();
    let mut bad = Vec::new();
    for (i, e) in adds.iter().enumerate() {
        on(Step::Percent(100.0 * i as f64 / adds.len().max(1) as f64));
        let f = bin.join(&e.path);
        if !f.is_file() || !download::sha256_matches(&f, e.sha256.as_deref().unwrap_or_default()) {
            bad.push(e.path.clone());
        }
    }
    Ok(bad)
}

/// Deletes a managed version. External installs are never deleted.
pub fn remove(v: &InstalledVersion, library: &str) -> Result<()> {
    if v.external {
        bail!(
            "{} was added from an existing folder; the launcher only forgets it.",
            v.name
        );
    }
    let root = Path::new(&v.root);
    let lib = Path::new(library);
    // Guard against a corrupted state file pointing somewhere unexpected.
    let inside = root.parent().is_some_and(|p| {
        paths::normalize(&p.to_string_lossy()) == paths::normalize(&lib.to_string_lossy())
    });
    if !inside || !root.join(paths::ARENA_DIR).is_dir() {
        bail!(
            "Refusing to delete {}: it is not a version folder inside the library.",
            v.root
        );
    }
    std::fs::remove_dir_all(root).with_context(|| format!("delete {}", v.root))
}

fn ensure_present(v: &InstalledVersion) -> Result<()> {
    if !paths::has_echo_install(&v.root) {
        bail!("Echo VR was not found at {}.", v.root);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_version(lib: &Path, id: &str) -> InstalledVersion {
        let root = lib.join(id);
        let bin = root.join("ready-at-dawn-echo-arena/bin/win10");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("echovr.exe"), b"").unwrap();
        InstalledVersion {
            id: id.into(),
            root: paths::normalize(&root.to_string_lossy()),
            ..Default::default()
        }
    }

    #[test]
    fn remove_only_managed_versions_inside_library() {
        let dir = tempfile::tempdir().unwrap();
        let lib = paths::normalize(&dir.path().join("lib").to_string_lossy());
        let v = fake_version(Path::new(&lib), "a");
        let mut ext = v.clone();
        ext.external = true;
        assert!(remove(&ext, &lib).is_err());
        let elsewhere = fake_version(dir.path(), "outside");
        assert!(remove(&elsewhere, &lib).is_err());
        assert!(Path::new(&elsewhere.root).exists());
        remove(&v, &lib).unwrap();
        assert!(!Path::new(&v.root).exists());
    }

    #[test]
    fn roots() {
        assert_eq!(
            root_for("C:\\EchoVR\\versions\\", "pc-1"),
            "C:/EchoVR/versions/pc-1"
        );
    }
}
