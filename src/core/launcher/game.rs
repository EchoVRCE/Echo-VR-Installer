//! Game state: is Echo running, is its local API up, is it in a match.
//!
//! A background thread polls once a second: an OS process scan for any build's executable
//! (which also sees games started outside the launcher, and Proton's wrappers) plus the
//! game's local HTTP API on port 6721. An HTTP error answer means "running, not in a
//! match"; a refused connection means "not running, or API access disabled".

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

/// Ends every game process (any build's executable, by name or Proton command line)
/// started at or after `since` (Unix seconds): the game the launcher started, even when
/// a wrapper (Revive's injector) started it, and never one started before. Returns how
/// many were ended.
pub fn stop_started_since(since: u64) -> usize {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet),
    );
    let names: Vec<String> = crate::core::paths::GAME_EXES
        .iter()
        .map(|e| e.to_ascii_lowercase())
        .collect();
    let mut ended = 0;
    for p in sys.processes().values() {
        let name = p.name().to_string_lossy().to_ascii_lowercase();
        let is_game = names.iter().any(|n| {
            name == *n
                || p.cmd().iter().any(|a| {
                    a.to_string_lossy()
                        .to_ascii_lowercase()
                        .ends_with(n.as_str())
                })
        });
        // Process start times are whole seconds: allow for the one we launched in.
        if is_game && p.start_time() + 1 >= since && p.kill() {
            tracing::info!("stopped {} (pid {})", name, p.pid());
            ended += 1;
        }
    }
    ended
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

/// Shared, continuously updated game state: on this PC, and on the Quest at its
/// network address (through its API) once that is known.
#[derive(Clone, Default)]
pub struct Monitor {
    state: Arc<Mutex<GameState>>,
    quest: Arc<Mutex<GameState>>,
    quest_ip: Arc<Mutex<Option<std::net::Ipv4Addr>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Stores `next` in `shared`; whether it changed.
fn store(shared: &Mutex<GameState>, next: GameState) -> bool {
    let mut s = lock(shared);
    let changed = *s != next;
    *s = next;
    changed
}

impl Monitor {
    /// Starts the polling threads; `on_change` runs whenever a state changes.
    pub fn start(on_change: impl Fn() + Send + Sync + 'static) -> Monitor {
        let m = Monitor::default();
        let on_change = Arc::new(on_change);
        let (quest, quest_ip, notify) = (m.quest.clone(), m.quest_ip.clone(), on_change.clone());
        std::thread::Builder::new()
            .name("quest-monitor".into())
            .spawn(move || loop {
                let ip = *lock(&quest_ip);
                let next = ip.map_or(GameState::NotRunning, super::quest_net::api_state);
                if store(&quest, next) {
                    notify();
                }
                std::thread::sleep(Duration::from_secs(2));
            })
            .expect("spawn quest monitor");
        let shared = m.state.clone();
        std::thread::Builder::new()
            .name("game-monitor".into())
            .spawn(move || loop {
                let process = crate::core::paths::GAME_EXES
                    .iter()
                    .any(|exe| process_running(exe));
                let api = if process { poll_api() } else { Err(()) };
                if store(&shared, interpret(api, process)) {
                    on_change();
                }
                std::thread::sleep(Duration::from_secs(1));
            })
            .expect("spawn game monitor");
        m
    }

    /// Where the Quest is on the network (`None`: not known, nothing polled).
    pub fn set_quest_ip(&self, ip: Option<std::net::Ipv4Addr>) {
        *lock(&self.quest_ip) = ip;
        if ip.is_none() {
            store(&self.quest, GameState::NotRunning);
        }
    }

    /// Echo VR on the Quest, from its API over the network.
    pub fn quest(&self) -> GameState {
        lock(&self.quest).clone()
    }

    pub fn get(&self) -> GameState {
        lock(&self.state).clone()
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
