//! The licence patch: a personal `pnsovr.dll` in `bin/win10`, in place of the game's
//! own. New players need it; owners may use it. The original is kept as
//! `pnsovr.dll.orig` so the patch can be taken off again.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::{anyhow, bail, Context, Result};

use super::versions::Step;
use crate::core::{download, oauth, paths};

pub const DLL: &str = "pnsovr.dll";
const ORIG: &str = "pnsovr.dll.orig";

/// Where the personal patch comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Authorize with Discord; the server builds a patch for this account.
    Discord,
    /// A patch link the user already has.
    Url(String),
}

#[derive(Debug)]
pub enum FetchError {
    OAuth(oauth::OAuthError),
    Other(anyhow::Error),
}

/// The staged patch in the download dir (deleted when the app exits).
pub fn staged() -> PathBuf {
    paths::downloads_dir().join(DLL)
}

/// Gets a personal patch file into the download dir.
pub fn fetch(
    source: &Source,
    cancel: &AtomicBool,
    on: &mut dyn FnMut(Step),
) -> Result<PathBuf, FetchError> {
    let url = match source {
        Source::Url(u) => oauth::validate_dll_url(u.trim()).ok_or_else(|| {
            FetchError::Other(anyhow!(
                "That link is not a licence patch link. Please check it and try again."
            ))
        })?,
        Source::Discord => oauth::run(oauth::FileType::Dll, cancel, &mut |s| on(Step::Status(s)))
            .map_err(FetchError::OAuth)?,
    };
    on(Step::Status("Downloading your patch...".into()));
    let job = download::Job {
        url,
        dir: paths::downloads_dir(),
        filename: DLL.into(),
        use_mirror: false,
        fresh: true,
        extract: false,
    };
    download::run(&job, cancel, &mut |_| {}).map_err(FetchError::Other)
}

/// Copies the patch into the install at `root`, backing up the original once.
pub fn apply(root: &str, dll: &Path) -> Result<()> {
    let bin = paths::bin_path(root);
    if !bin.is_dir() {
        bail!("Couldn't find ready-at-dawn-echo-arena\\bin\\win10 in {root}.");
    }
    let dst = bin.join(DLL);
    let orig = bin.join(ORIG);
    if dst.is_file() && !orig.exists() {
        std::fs::copy(&dst, &orig).context("Couldn't back up the original pnsovr.dll")?;
    }
    std::fs::copy(dll, &dst).with_context(|| {
        format!(
            "Couldn't write {}. If the game is in Program Files, try running the launcher as administrator.",
            dst.display()
        )
    })?;
    tracing::info!("licence patch applied in {root}");
    Ok(())
}

/// Puts the original `pnsovr.dll` back.
pub fn remove(root: &str) -> Result<()> {
    let bin = paths::bin_path(root);
    let orig = bin.join(ORIG);
    if !orig.is_file() {
        bail!(
            "There is no original pnsovr.dll to restore. Verify or reinstall the version instead."
        );
    }
    std::fs::rename(&orig, bin.join(DLL)).context("Couldn't restore the original pnsovr.dll")?;
    tracing::info!("licence patch removed in {root}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_and_remove_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_string_lossy().into_owned();
        assert!(
            apply(&root, Path::new("/nonexistent")).is_err(),
            "no bin dir"
        );
        let bin = paths::bin_path(&root);
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join(DLL), b"original").unwrap();
        let patch = dir.path().join("patch.dll");
        std::fs::write(&patch, b"patched").unwrap();

        apply(&root, &patch).unwrap();
        assert_eq!(std::fs::read(bin.join(DLL)).unwrap(), b"patched");
        assert_eq!(std::fs::read(bin.join(ORIG)).unwrap(), b"original");
        // A second patch keeps the first backup.
        apply(&root, &patch).unwrap();
        assert_eq!(std::fs::read(bin.join(ORIG)).unwrap(), b"original");

        remove(&root).unwrap();
        assert_eq!(std::fs::read(bin.join(DLL)).unwrap(), b"original");
        assert!(!bin.join(ORIG).exists());
        assert!(remove(&root).is_err(), "nothing to restore");
    }

    #[test]
    fn updates_skip_the_patch() {
        use crate::core::pc_update::skipped;
        assert!(skipped("pnsovr.dll", &[DLL]));
        assert!(skipped("PNSOVR.DLL", &[DLL]));
        assert!(!skipped("echovr.exe", &[DLL]));
        assert!(!skipped("pnsovr.dll", &[]));
    }
}
