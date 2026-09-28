//! Zip extraction. Every entry name is resolved with `enclosed_name()`, so an entry like
//! `../../Windows/System32/x.dll` (Zip Slip) is rejected instead of written outside the
//! destination -- the Java `UnzipFile` concatenated names blindly, which mattered most for
//! the artwork zip the elevated helper extracts into Program Files.

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{bail, Context, Result};

pub fn extract(zip_path: &Path, dest: &Path, cancel: &AtomicBool) -> Result<usize> {
    let file = File::open(zip_path).with_context(|| format!("open {}", zip_path.display()))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("{} is not a valid zip file", zip_path.display()))?;
    std::fs::create_dir_all(dest).with_context(|| format!("create {}", dest.display()))?;
    tracing::info!("extract {} -> {}", zip_path.display(), dest.display());

    let mut files = 0;
    let mut buf = vec![0u8; 1 << 16];
    for i in 0..archive.len() {
        if cancel.load(Ordering::Relaxed) {
            return Err(super::http::Cancelled.into());
        }
        let mut entry = archive.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else {
            bail!("The archive contains an unsafe path: {}", entry.name());
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).with_context(|| format!("create {}", out.display()))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let f = File::create(&out).with_context(|| format!("write {}", out.display()))?;
        let mut w = BufWriter::new(f);
        loop {
            let n = entry.read(&mut buf)?;
            if n == 0 {
                break;
            }
            w.write_all(&buf[..n])
                .with_context(|| format!("write {}", out.display()))?;
        }
        w.flush()?;
        files += 1;
    }
    tracing::info!("extracted {files} file(s)");
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::SimpleFileOptions;

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        let f = File::create(path).unwrap();
        let mut z = zip::ZipWriter::new(f);
        for (name, data) in entries {
            z.start_file(*name, SimpleFileOptions::default()).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn extracts_nested_files() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("a.zip");
        make_zip(&zip, &[("a/b/c.txt", b"hi"), ("top.txt", b"x")]);
        let out = dir.path().join("out");
        assert_eq!(extract(&zip, &out, &AtomicBool::new(false)).unwrap(), 2);
        assert_eq!(std::fs::read(out.join("a/b/c.txt")).unwrap(), b"hi");
    }

    #[test]
    fn rejects_zip_slip() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("evil.zip");
        make_zip(&zip, &[("../escaped.txt", b"pwned")]);
        let out = dir.path().join("out");
        assert!(extract(&zip, &out, &AtomicBool::new(false)).is_err());
        assert!(!dir.path().join("escaped.txt").exists());
    }
}
