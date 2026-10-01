//! What Echo VR needs on Linux, the way EchoXR does it (github.com/EchoTools/EchoXR, its
//! OpenXR layer only): a private GE-Proton, whose `wineopenxr` passes OpenXR on to the
//! runtime the system uses (SteamVR, Monado, WiVRn); a Wine prefix; and EchoXR's runtime
//! in the game's `bin/win10`, which answers Echo's LibOVR calls over OpenXR. `EchoXR.exe`
//! starts the game there: no Meta services, sign-in or store, and no injection. Meta's
//! `LibOVRPlatform64_1.dll` (for `pnsovr.dll`'s login) is read out of Meta's own runtime
//! package, never shipped. Everything else lives in `<data dir>/linux`; every download
//! is pinned by its SHA-256.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::{bail, Context, Result};

use crate::core::download::{self, Progress};
use crate::core::launcher::versions::Step;
use crate::core::{paths, remote_zip};

/// GE-Proton, pinned; it carries `wineopenxr`.
pub const PROTON: &str = "GE-Proton11-3";
const PROTON_URL: &str = "https://github.com/GloriousEggroll/proton-ge-custom/releases/download/GE-Proton11-3/GE-Proton11-3.tar.gz";
const PROTON_SHA256: &str = "861c2edc8d40d051fb1e7a692deb953be52bd339c46d90f2b7dde50ddad91266";

/// EchoXR's OpenXR layer (EchoTools/EchoXR 0.3.0), as files.echovr.de hosts it: unpacked
/// into the game's `bin/win10`.
pub const ECHOXR: &str = "0.3.0";
const ECHOXR_ZIP: &str = "EchoXR-OpenXR-v0.3.0.zip";
const ECHOXR_URL: &str = "https://files.echovr.de/EchoXR-OpenXR-v0.3.0.zip";
const ECHOXR_SHA256: &str = "1e14c92c586d73eef40f179ea1b926c3bc99157a40b7675d7e6f0186d4c93447";
/// The files that do the work, with their hashes (the zip's manifest has them too).
const ECHOXR_FILES: [(&str, &str); 3] = [
    (
        "EchoXR.exe",
        "628c00ffb343feace777ddc4c221932caf1b55d4cfbaa41492c45c2d6e937a35",
    ),
    (
        "EchoXR/LibOVRRT64_1.dll",
        "aaed21b09778713103ad0817f4af9e0cbb1b765e979fb74d7d4e399e8dfa35f6",
    ),
    (
        "EchoXR/openxr_loader.dll",
        "711bf48c5c4141f9a03d4dcd69731cc9f8983c353111c74aaeefa979e43df441",
    ),
];
/// The launcher keeps EchoXR at the version it pins: its own updater stays off (it
/// couldn't run under Wine anyway).
const ECHOXR_INI: &str =
    "# Kept by the Echo VR launcher, which updates EchoXR itself\r\nCheckForUpdates = 0\r\n";

/// Meta's PC runtime package; only its Platform SDK loader is read out of it.
const META_RUNTIME_URL: &str =
    "https://securecdn.oculus.com/binaries/download/?id=3766757683456363";
const PLATFORM: (&str, &str) = (
    "LibOVRPlatform64_1.dll",
    "c8f99087f457bed5c0549da82d7b976d6687017c72ddc25277babadaab16001b",
);

const MARKER: &str = "setup.json";

/// Where everything goes.
pub fn root() -> PathBuf {
    paths::data_dir().join("linux")
}

fn proton_dir() -> PathBuf {
    root().join(PROTON)
}

fn compat_dir() -> PathBuf {
    root().join("compatdata")
}

fn prefix() -> PathBuf {
    compat_dir().join("pfx")
}

fn echoxr_zip() -> PathBuf {
    root().join(ECHOXR_ZIP)
}

fn platform_dll() -> PathBuf {
    root().join("oculus").join(PLATFORM.0)
}

/// What this launcher's setup consists of: a different one means setting up again.
fn marker_text() -> String {
    format!("{{\"proton\":\"{PROTON}\",\"echoxr\":\"{ECHOXR}\"}}")
}

/// Whether the setup is done (and still all there).
pub fn is_set_up() -> bool {
    std::fs::read_to_string(root().join(MARKER)).is_ok_and(|m| m == marker_text())
        && proton_dir().join("proton").is_file()
        && echoxr_zip().is_file()
        && platform_dll().is_file()
}

/// The OpenXR runtime games are to use: `XR_RUNTIME_JSON` when set, else the system's
/// active one.
pub fn openxr_runtime() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("XR_RUNTIME_JSON").map(PathBuf::from) {
        return p.is_file().then_some(p);
    }
    let config = config_dir()?;
    [
        config.join("openxr/1/active_runtime.json"),
        PathBuf::from("/etc/xdg/openxr/1/active_runtime.json"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

fn config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
}

/// The OpenVR runtime Proton loads (`bin/linux64/vrclient.so`) before it turns OpenXR on
/// for a game: `VR_OVERRIDE`, else the first one registered in `openvrpaths.vrpath`.
pub fn openvr_runtime() -> Option<PathBuf> {
    let dir = match std::env::var_os("VR_OVERRIDE") {
        Some(d) => PathBuf::from(d),
        None => {
            let vrpath = std::env::var_os("VR_PATHREG_OVERRIDE")
                .map(PathBuf::from)
                .or_else(|| config_dir().map(|c| c.join("openvr/openvrpaths.vrpath")))?;
            first_runtime(&std::fs::read_to_string(vrpath).ok()?)?
        }
    };
    dir.join("bin/linux64/vrclient.so").is_file().then_some(dir)
}

/// Pure: the first `"runtime"` folder in an `openvrpaths.vrpath`.
fn first_runtime(vrpath: &str) -> Option<PathBuf> {
    let v: serde_json::Value = serde_json::from_str(vrpath).ok()?;
    v.get("runtime")?
        .as_array()?
        .first()?
        .as_str()
        .map(PathBuf::from)
}

/// The environment Proton runs with (for setup steps and the game).
fn proton_env(steam_root: &Path, game_dir: Option<&Path>) -> Vec<(String, String)> {
    let s = |p: &Path| p.to_string_lossy().into_owned();
    let mut env = vec![
        ("STEAM_COMPAT_DATA_PATH".into(), s(&compat_dir())),
        ("STEAM_COMPAT_CLIENT_INSTALL_PATH".into(), s(steam_root)),
        (
            "STEAM_COMPAT_LIBRARY_PATHS".into(),
            s(&steam_root.join("steamapps")),
        ),
        ("SteamAppId".into(), "0".into()),
        ("SteamGameId".into(), "0".into()),
        ("UMU_ID".into(), "umu-default".into()),
        ("UMU_USE_STEAM".into(), "0".into()),
        ("PROTON_LOG".into(), "0".into()),
        ("WINEDEBUG".into(), "-all".into()),
    ];
    if let Some(dir) = game_dir {
        env.push(("STEAM_COMPAT_INSTALL_PATH".into(), s(dir)));
    }
    env
}

/// Runs `proton <verb> <args>` and waits.
fn proton(steam_root: &Path, verb: &str, args: &[&str]) -> Result<()> {
    let status = std::process::Command::new(proton_dir().join("proton"))
        .arg(verb)
        .args(args)
        .envs(proton_env(steam_root, None))
        .stdin(std::process::Stdio::null())
        .status()
        .with_context(|| format!("couldn't run Proton ({verb})"))?;
    if !status.success() {
        bail!(
            "Proton {verb} {} failed ({status})",
            args.first().unwrap_or(&"")
        );
    }
    Ok(())
}

/// Downloads `url` into the cache as `name` and checks it against `sha256`.
fn fetch(
    url: &str,
    name: &str,
    sha256: &str,
    cancel: &AtomicBool,
    on: &mut dyn FnMut(Step),
) -> Result<PathBuf> {
    let job = download::Job {
        url: url.into(),
        dir: paths::downloads_dir().join("linux"),
        filename: name.into(),
        use_mirror: false,
        fresh: false,
        extract: false,
    };
    let file = download::run(&job, cancel, &mut |p| {
        if let Progress::Percent(v) = p {
            on(Step::Percent(v));
        }
    })?;
    if !download::sha256_matches(&file, sha256) {
        let _ = std::fs::remove_file(&file);
        bail!("{name} didn't download correctly (checksum mismatch). Please try again.");
    }
    Ok(file)
}

/// Sets everything up (each step skipped when already done): GE-Proton, EchoXR, Meta's
/// Platform SDK loader, and the prefix.
pub fn setup(steam_root: &Path, cancel: &AtomicBool, on: &mut dyn FnMut(Step)) -> Result<()> {
    std::fs::create_dir_all(root())?;
    let _ = std::fs::remove_file(root().join(MARKER));

    if !proton_dir().join("proton").is_file() {
        on(Step::Status(format!("Downloading {PROTON}...")));
        let archive = fetch(
            PROTON_URL,
            &format!("{PROTON}.tar.gz"),
            PROTON_SHA256,
            cancel,
            on,
        )?;
        on(Step::Status(format!("Unpacking {PROTON}...")));
        let gz = flate2::read::GzDecoder::new(std::fs::File::open(&archive)?);
        tar::Archive::new(gz)
            .unpack(root())
            .with_context(|| format!("unpack {PROTON}"))?;
        let _ = std::fs::remove_file(&archive);
    }
    if !proton_dir()
        .join("files/lib/wine/x86_64-windows/wineopenxr.dll")
        .is_file()
    {
        bail!("{PROTON} has no OpenXR bridge (wineopenxr), which EchoXR needs");
    }

    if !download::sha256_matches(&echoxr_zip(), ECHOXR_SHA256) {
        on(Step::Status(format!("Downloading EchoXR {ECHOXR}...")));
        let zip = fetch(ECHOXR_URL, ECHOXR_ZIP, ECHOXR_SHA256, cancel, on)?;
        std::fs::copy(&zip, echoxr_zip()).context("keep EchoXR")?;
        let _ = std::fs::remove_file(zip);
    }

    if !download::sha256_matches(&platform_dll(), PLATFORM.1) {
        on(Step::Status("Reading Meta's Platform SDK loader...".into()));
        let dir = root().join("oculus");
        let member = [(PLATFORM.0.to_string(), PLATFORM.1.to_string())];
        remote_zip::extract_members(META_RUNTIME_URL, "", &member, &dir, cancel, &mut |_, _| {})
            .context("Couldn't read LibOVRPlatform64_1.dll out of Meta's runtime package")?;
    }

    on(Step::Status("Preparing the Wine prefix...".into()));
    std::fs::create_dir_all(compat_dir())?;
    proton(steam_root, "runinprefix", &["cmd.exe", "/c", "exit"])?;
    // Let the prefix settle before the game first uses it.
    let _ = std::process::Command::new(proton_dir().join("files/bin/wineserver"))
        .args(["-k", "-w"])
        .env("WINEPREFIX", prefix())
        .status();
    std::fs::write(root().join(MARKER), marker_text())?;
    Ok(())
}

/// Puts EchoXR into the game's bin folder `bin` (next to `echovr.exe`), with Meta's
/// Platform SDK loader and the launcher's `echoxr.ini`: only what is missing or differs.
pub fn install_into(bin: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(
        std::fs::File::open(echoxr_zip()).context("EchoXR isn't set up: set up Linux again")?,
    )?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let Some(rel) = entry.enclosed_name() else {
            bail!("EchoXR's zip has an unsafe path: {}", entry.name());
        };
        let out = bin.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut entry, &mut bytes)?;
        if let Some((_, sha)) = ECHOXR_FILES.iter().find(|(n, _)| Path::new(n) == rel) {
            if !download::sha256_reader(&mut bytes.as_slice())?.eq_ignore_ascii_case(sha) {
                bail!("{} in EchoXR's zip isn't the expected build", rel.display());
            }
        }
        if std::fs::read(&out).is_ok_and(|have| have == bytes) {
            continue;
        }
        put(&out, &bytes)?;
    }
    let ini = bin.join("EchoXR/echoxr.ini");
    if std::fs::read(&ini).map_or(true, |have| have != ECHOXR_INI.as_bytes()) {
        put(&ini, ECHOXR_INI.as_bytes())?;
    }
    let platform = bin.join(PLATFORM.0);
    if !download::sha256_matches(&platform, PLATFORM.1) {
        std::fs::copy(platform_dll(), &platform).context("copy LibOVRPlatform64_1.dll")?;
    }
    Ok(())
}

/// Writes `bytes` to `path` (making its folder).
fn put(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

/// The command that starts Echo VR from its bin folder `bin` through Proton and
/// `EchoXR.exe` (which makes `echovr_openxr.exe` on its first run and points Echo at the
/// runtime), with `args` for the game.
pub fn game_command(
    steam_root: &Path,
    bin: &Path,
    args: &[String],
) -> Result<std::process::Command> {
    let game_root = bin
        .ancestors()
        .find(|p| p.file_name().is_some_and(|n| n == paths::ARENA_DIR))
        .unwrap_or(bin)
        .to_path_buf();
    let xr = openxr_runtime().context(
        "No OpenXR runtime is set up on this PC. Start SteamVR, Monado or WiVRn once (or set XR_RUNTIME_JSON), then try again.",
    )?;
    if openvr_runtime().is_none() {
        bail!(
            "No OpenVR runtime is registered, and Proton needs one before it turns OpenXR on. SteamVR: start it once. Monado or WiVRn: install xrizer or OpenComposite and register it."
        );
    }
    install_into(bin)?;
    let mut rw = format!("{}:{}", game_root.display(), prefix().display());
    if let Ok(more) = std::env::var("PRESSURE_VESSEL_FILESYSTEMS_RW") {
        rw = format!("{rw}:{more}");
    }
    let mut c = std::process::Command::new(proton_dir().join("proton"));
    c.arg("waitforexitandrun")
        .arg(bin.join("EchoXR.exe"))
        .args(args)
        .current_dir(bin)
        .envs(proton_env(steam_root, Some(&game_root)))
        .env("XR_RUNTIME_JSON", xr)
        .env("PRESSURE_VESSEL_IMPORT_OPENXR_1_RUNTIMES", "1")
        .env("PRESSURE_VESSEL_FILESYSTEMS_RW", rw)
        // dbgcore.dll in bin/win10 is the game's plugin loader (the community update ships
        // it); Wine would load its own instead.
        .env("WINEDLLOVERRIDES", "dbgcore=n,b");
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_openvr_registration() {
        let vrpath = r#"{"config":["/x"],"runtime":["/home/u/.steam/steam/steamapps/common/SteamVR"],"version":1}"#;
        assert_eq!(
            first_runtime(vrpath),
            Some(PathBuf::from(
                "/home/u/.steam/steam/steamapps/common/SteamVR"
            ))
        );
        assert_eq!(first_runtime(r#"{"runtime":[]}"#), None);
        assert_eq!(first_runtime("not json"), None);
    }

    /// The pinned EchoXR zip is on files.echovr.de, its files are the pinned builds, and
    /// it unpacks into a game folder with the launcher's echoxr.ini.
    #[test]
    #[ignore = "network"]
    fn installs_the_pinned_echoxr() {
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join(ECHOXR_ZIP);
        crate::core::http::download_to(ECHOXR_URL, &zip, None).unwrap();
        assert!(download::sha256_matches(&zip, ECHOXR_SHA256));
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&zip).unwrap()).unwrap();
        for (name, sha) in ECHOXR_FILES {
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut archive.by_name(name).unwrap(), &mut bytes).unwrap();
            assert_eq!(
                download::sha256_reader(&mut bytes.as_slice()).unwrap(),
                sha,
                "{name}"
            );
        }
    }

    /// Meta's package still has the pinned Platform SDK loader, and it can be read out of
    /// it alone.
    #[test]
    #[ignore = "network"]
    fn reads_the_platform_loader_out_of_metas_package() {
        let dir = tempfile::tempdir().unwrap();
        let member = [(PLATFORM.0.to_string(), PLATFORM.1.to_string())];
        remote_zip::extract_members(
            META_RUNTIME_URL,
            "",
            &member,
            dir.path(),
            &AtomicBool::new(false),
            &mut |_, _| {},
        )
        .unwrap();
        assert!(download::sha256_matches(
            &dir.path().join(PLATFORM.0),
            PLATFORM.1
        ));
    }
}
