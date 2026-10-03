//! What Echo VR needs on Linux, the way EchoXR does it (github.com/EchoTools/EchoXR, its
//! OpenXR layer only): a private GE-Proton, whose `wineopenxr` passes OpenXR on to the
//! runtime the system uses (SteamVR, Monado, WiVRn); a Wine prefix; and EchoXR
//! (`core::echoxr`) in the game's `bin/win10`, with Meta's Platform SDK loader beside the
//! game (a prefix has no Meta app). Everything lives in `<data dir>/linux`; every
//! download is pinned by its SHA-256.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::{bail, Context, Result};

use crate::core::echoxr;
use crate::core::launcher::versions::Step;
use crate::core::paths;

/// GE-Proton, pinned; it carries `wineopenxr`.
pub const PROTON: &str = "GE-Proton11-3";
const PROTON_URL: &str = "https://github.com/GloriousEggroll/proton-ge-custom/releases/download/GE-Proton11-3/GE-Proton11-3.tar.gz";
const PROTON_SHA256: &str = "861c2edc8d40d051fb1e7a692deb953be52bd339c46d90f2b7dde50ddad91266";

const MARKER: &str = "setup.json";
/// Which of Meta's DLLs the setup brings (3: the Platform SDK loader and its P2P library;
/// an older setup fetches what's new).
const PLATFORM_REV: u32 = 3;

/// Where everything goes (EchoXR's zip and Meta's loader too).
pub fn root() -> PathBuf {
    echoxr::store_dir()
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

/// What this launcher's setup consists of: a different one means setting up again.
fn marker_text() -> String {
    format!(
        "{{\"proton\":\"{PROTON}\",\"echoxr\":\"{}\",\"platform\":{PLATFORM_REV}}}",
        echoxr::VERSION
    )
}

/// Whether the setup is done (and still all there).
pub fn is_set_up() -> bool {
    std::fs::read_to_string(root().join(MARKER)).is_ok_and(|m| m == marker_text())
        && proton_dir().join("proton").is_file()
        && echoxr::is_fetched()
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

/// Whether the OpenXR runtime's service runs: SteamVR's `vrserver`, or Monado's or
/// WiVRn's socket. EchoXR is told so; without it, it gives up after 20 s (exit code 5)
/// rather than hanging when there is none.
fn vr_service_running() -> bool {
    let xdg = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    vr_service_in(
        xdg.as_deref(),
        crate::core::launcher::game::process_running("vrserver"),
    )
}

/// Pure but for the file checks: a VR service, given the runtime folder and whether
/// `vrserver` runs.
fn vr_service_in(xdg_runtime_dir: Option<&Path>, vrserver: bool) -> bool {
    vrserver
        || xdg_runtime_dir.is_some_and(|d| {
            d.join("monado_comp_ipc").exists() || d.join("wivrn/comp_ipc").exists()
        })
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

/// Sets everything up (each step skipped when already done): GE-Proton, EchoXR, Meta's
/// Platform SDK loader, and the prefix.
pub fn setup(steam_root: &Path, cancel: &AtomicBool, on: &mut dyn FnMut(Step)) -> Result<()> {
    std::fs::create_dir_all(root())?;
    let _ = std::fs::remove_file(root().join(MARKER));

    if !proton_dir().join("proton").is_file() {
        on(Step::Status(format!("Downloading {PROTON}...")));
        let archive = echoxr::fetch_pinned(
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

    echoxr::fetch(cancel, on)?;

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

/// The command that starts Echo VR from its bin folder `bin` through Proton and
/// `EchoXR.exe` (which makes `echovr_openxr.exe` on its first run, turns Proton's OpenXR on
/// itself -- no OpenVR runtime needed -- and points Echo at the runtime), with `args` for
/// the game.
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
    // Meta's loader and P2P library beside the game: a prefix has no Meta app to bring
    // them.
    echoxr::install_into(bin, Some(bin))?;
    echoxr::refresh_openxr_exe(bin)?;
    let mut rw = format!("{}:{}", game_root.display(), prefix().display());
    if let Ok(more) = std::env::var("PRESSURE_VESSEL_FILESYSTEMS_RW") {
        rw = format!("{rw}:{more}");
    }
    let mut c = std::process::Command::new(proton_dir().join("proton"));
    c.arg("waitforexitandrun")
        .arg(bin.join(echoxr::LAUNCHER))
        .args(args)
        .current_dir(bin)
        .envs(proton_env(steam_root, Some(&game_root)))
        .env("XR_RUNTIME_JSON", xr)
        .env("PRESSURE_VESSEL_IMPORT_OPENXR_1_RUNTIMES", "1")
        .env("PRESSURE_VESSEL_FILESYSTEMS_RW", rw);
    // The mod loader is the game's BugSplat64.dll, which Wine has no builtin of, so the
    // game's own folder wins anyway. Only the old loader, EchoLoader 1, was dbgcore.dll:
    // Wine would load its own dbgcore instead of that one.
    if crate::core::launcher::mods::legacy_in(bin) {
        c.env("WINEDLLOVERRIDES", "dbgcore=n,b");
    }
    if vr_service_running() {
        c.env("ECHOXR_VR_SERVICE", "ready");
    } else {
        tracing::warn!("--play: no VR service seen (SteamVR, monado-service, wivrn-server)");
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sees_the_vr_service() {
        let dir = tempfile::tempdir().unwrap();
        assert!(vr_service_in(None, true));
        assert!(!vr_service_in(None, false));
        assert!(!vr_service_in(Some(dir.path()), false));
        std::fs::create_dir_all(dir.path().join("wivrn")).unwrap();
        std::fs::write(dir.path().join("wivrn/comp_ipc"), b"").unwrap();
        assert!(vr_service_in(Some(dir.path()), false));
    }
}
