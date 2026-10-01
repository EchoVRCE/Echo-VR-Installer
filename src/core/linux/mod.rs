//! Echo VR for PC on Linux: added to Steam as a non-Steam game that runs the launcher
//! (`--play`), which starts the game through a private GE-Proton with EchoXR's OpenXR
//! runtime standing in for Meta's.

pub mod echoxr;
pub mod steam;
pub mod vdf;

use std::path::PathBuf;

use crate::core::launcher::launch;
use crate::core::launcher::store::{LauncherState, Target};

/// What Steam's shortcut runs the launcher with.
pub const PLAY_FLAG: &str = "--play";

/// Where a lobby to join waits for the next `--play` (Steam's link can't carry it).
fn next_lobby_file() -> PathBuf {
    echoxr::root().join("next-lobby")
}

/// Has the next start through Steam join `lobby` (and watch it as a spectator), or not.
pub fn set_next_lobby(lobby: Option<(&str, bool)>) {
    let f = next_lobby_file();
    match lobby {
        Some((l, spectate)) => {
            let _ = std::fs::create_dir_all(echoxr::root());
            let _ = std::fs::write(
                f,
                if spectate {
                    format!("s:{l}")
                } else {
                    l.to_string()
                },
            );
        }
        None => {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// The lobby left for this start, and whether to watch it.
fn take_next_lobby() -> Option<(String, bool)> {
    let f = next_lobby_file();
    let text = std::fs::read_to_string(&f).ok();
    let _ = std::fs::remove_file(f);
    let text = text?;
    let (spectate, id) = match text.strip_prefix("s:") {
        Some(id) => (true, id),
        None => (false, text.as_str()),
    };
    launch::lobby_uuid(id).map(|l| (l, spectate))
}

/// The launcher's own executable, as Steam's shortcut should run it (the AppImage when
/// it runs from one).
pub fn launcher_exe() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
}

/// `--play`: starts the version PLAY would start, with the launch options, through Proton
/// and EchoXR, and waits for the game to end. Returns the exit code.
pub fn play_from_steam() -> i32 {
    let state = LauncherState::load();
    let Target::Installed(v) = state.target(None, |v| v.present()) else {
        tracing::error!("--play: no installed version to start");
        return 2;
    };
    let Some(steam_root) = steam::root() else {
        tracing::error!("--play: Steam not found");
        return 2;
    };
    let lobby = take_next_lobby();
    let mut profile = state.profile.clone();
    if lobby.as_ref().is_some_and(|(_, spectate)| *spectate) {
        profile.runtime = crate::core::launcher::store::Runtime::Flat;
        profile.spectator = true;
    }
    // Event builds start bare (the 2019 one quits on flags it doesn't know).
    let args = if v.publisher_lock.is_some() {
        Vec::new()
    } else {
        launch::game_args(&profile, lobby.as_ref().map(|(l, _)| l.as_str()))
    };
    // EchoXR runs only the live build's echovr.exe (its patch checks the bytes first).
    if v.publisher_lock.is_some() {
        tracing::error!("--play: event builds don't run on Linux yet");
        return 2;
    }
    match echoxr::game_command(&steam_root, &v.bin_dir(), &args).and_then(|mut c| {
        tracing::info!("--play: {c:?}");
        Ok(c.status()?)
    }) {
        Ok(status) => status.code().unwrap_or(0),
        Err(e) => {
            tracing::error!("--play: {e:#}");
            1
        }
    }
}
