//! Game state: is Echo running, is its local API up, is it in a match.
//!
//! A background thread polls once a second: an OS process scan (which also sees games
//! started outside the launcher, and Proton's `echovr.exe` wrappers) plus the game's local
//! HTTP API on port 6721. An HTTP error answer means "running, not in a match"; a refused
//! connection means "not running, or API access disabled".

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Deserialize;

pub const API_URL: &str = "http://127.0.0.1:6721/session";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GameState {
    #[default]
    NotRunning,
    /// Running but the local API is off (`EnableAPIAccess` false) or still starting.
    RunningApiOff,
    /// Running, not in a match (menus / lobby loading).
    Running,
    InMatch {
        session_id: String,
    },
}

impl GameState {
    pub fn is_running(&self) -> bool {
        *self != GameState::NotRunning
    }

    pub fn label(&self) -> String {
        match self {
            GameState::NotRunning => "Echo VR is not running".into(),
            GameState::RunningApiOff => "Echo VR is running".into(),
            GameState::Running => "Echo VR is running (in the menus)".into(),
            GameState::InMatch { session_id } => format!("In a match ({})", short(session_id)),
        }
    }
}

fn short(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

/// True when a process with this executable name (case-insensitive) is running. On
/// Linux, Proton's game process can have an empty name, so the command line is checked too.
pub fn process_running(exe_name: &str) -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet),
    );
    let want = exe_name.to_ascii_lowercase();
    sys.processes().values().any(|p| {
        p.name().to_string_lossy().to_ascii_lowercase() == want
            || p.cmd()
                .iter()
                .any(|a| a.to_string_lossy().to_ascii_lowercase().ends_with(&want))
    })
}

#[derive(Deserialize)]
struct Session {
    #[serde(default)]
    sessionid: String,
    #[serde(default)]
    err_code: Option<i64>,
}

/// Pure: interprets one API poll. `Ok((status, body))` is an HTTP answer.
pub fn interpret(api: Result<(u16, String), ()>, process: bool) -> GameState {
    match api {
        Ok((200, body)) => match serde_json::from_str::<Session>(&body) {
            Ok(s) if s.err_code.unwrap_or(0) == 0 && !s.sessionid.is_empty() => {
                GameState::InMatch {
                    session_id: s.sessionid,
                }
            }
            _ => GameState::Running,
        },
        Ok(_) => GameState::Running,
        Err(()) if process => GameState::RunningApiOff,
        Err(()) => GameState::NotRunning,
    }
}

fn poll_api() -> Result<(u16, String), ()> {
    crate::core::http::block_on(async {
        let resp = crate::core::http::client()
            .get(API_URL)
            .timeout(Duration::from_millis(800))
            .send()
            .await
            .map_err(|_| ())?;
        let status = resp.status().as_u16();
        Ok((status, resp.text().await.unwrap_or_default()))
    })
}

/// Shared, continuously updated game state.
#[derive(Clone, Default)]
pub struct Monitor {
    state: Arc<Mutex<GameState>>,
}

impl Monitor {
    /// Starts the polling thread; `on_change` runs whenever the state changes.
    pub fn start(on_change: impl Fn() + Send + 'static) -> Monitor {
        let m = Monitor::default();
        let shared = m.state.clone();
        std::thread::Builder::new()
            .name("game-monitor".into())
            .spawn(move || loop {
                let process = process_running("echovr.exe");
                let api = if process { poll_api() } else { Err(()) };
                let next = interpret(api, process);
                let changed = {
                    let mut s = shared.lock().unwrap_or_else(|p| p.into_inner());
                    let changed = *s != next;
                    *s = next;
                    changed
                };
                if changed {
                    on_change();
                }
                std::thread::sleep(Duration::from_secs(1));
            })
            .expect("spawn game monitor");
        m
    }

    pub fn get(&self) -> GameState {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_ladder() {
        assert_eq!(interpret(Err(()), false), GameState::NotRunning);
        assert_eq!(interpret(Err(()), true), GameState::RunningApiOff);
        assert_eq!(
            interpret(Ok((404, String::new())), true),
            GameState::Running
        );
        assert_eq!(
            interpret(Ok((200, r#"{"err_code":-6}"#.into())), true),
            GameState::Running
        );
        assert_eq!(
            interpret(
                Ok((
                    200,
                    r#"{"sessionid":"ABCDEF12-0000","game_status":"playing"}"#.into()
                )),
                true
            ),
            GameState::InMatch {
                session_id: "ABCDEF12-0000".into()
            }
        );
        assert_eq!(
            GameState::InMatch {
                session_id: "ABCDEF1234".into()
            }
            .label(),
            "In a match (ABCDEF12)"
        );
    }
}
