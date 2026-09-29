//! Starting the PC game with a launch profile.
//!
//! Commands are built as argv (never through a shell) and the game's output is discarded
//! -- the old launcher piped stderr without reading it, which can stall the game.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use super::store::{LaunchProfile, Runtime};
use crate::core::revive;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

/// Splits extra arguments like a command line: whitespace separates, double quotes group.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut any = false;
    for c in s.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    out.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        out.push(cur);
    }
    out
}

/// A lobby id as the game expects it: a UUID (any `.node` suffix dropped).
// Unused while the Play page has no lobby field (it comes back with the next design pass).
#[allow(dead_code)]
pub fn lobby_uuid(input: &str) -> Option<String> {
    let s = input.trim();
    // Accept pasted spark:// / echo.taxi links: the UUID is the last path segment.
    let last = s.rsplit('/').next().unwrap_or(s);
    let id = last.split('.').next().unwrap_or(last);
    let hex: String = id.chars().filter(|c| *c != '-').collect();
    (hex.len() == 32 && hex.chars().all(|c| c.is_ascii_hexdigit()) && id.len() == 36)
        .then(|| id.to_ascii_uppercase())
}

/// The game arguments for a profile (without the executable).
pub fn game_args(profile: &LaunchProfile, lobby: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    if profile.runtime == Runtime::Flat {
        args.push("-noovr".to_string());
        if profile.spectator {
            args.push("-spectatorstream".into());
        }
    }
    if profile.windowed {
        args.push("-windowed".into());
    }
    if let Some(l) = lobby {
        args.push("-lobbyid".into());
        args.push(l.to_string());
    }
    args.extend(split_args(&profile.extra_args));
    args
}

pub fn build(
    profile: &LaunchProfile,
    exe: &Path,
    revive_dir: Option<&str>,
    lobby: Option<&str>,
) -> Result<Command> {
    let cwd = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    let args = game_args(profile, lobby);
    match profile.runtime {
        Runtime::MetaLink | Runtime::VirtualDesktop | Runtime::Flat => Ok(Command {
            program: exe.to_path_buf(),
            args,
            cwd,
        }),
        Runtime::Revive => {
            let Some(dir) = revive_dir else {
                bail!("Revive is not installed. Set up SteamVR from the PLAY button first.");
            };
            let mut a = vec![exe.to_string_lossy().into_owned(), "-nosymbollookup".into()];
            a.extend(args);
            a.push("/app".into());
            a.push(revive::APP_ID.into());
            Ok(Command {
                program: Path::new(dir).join(revive::REVIVE_INJECTOR),
                args: a,
                cwd: PathBuf::from(dir),
            })
        }
    }
}

/// Things worth warning about before launching (not errors: the user may know better).
pub fn preflight(profile: &LaunchProfile) -> Option<String> {
    let running = |name: &str| super::game::process_running(name);
    match profile.runtime {
        Runtime::MetaLink if cfg!(windows) && !running("OVRServer_x64.exe") => Some(
            "The Meta Quest Link app (Oculus runtime) doesn't seem to be running.\nStart it and connect your headset first."
                .into(),
        ),
        Runtime::VirtualDesktop if cfg!(windows) && !running("VirtualDesktop.Streamer.exe") => Some(
            "The Virtual Desktop Streamer doesn't seem to be running.\nStart it and connect from your headset first."
                .into(),
        ),
        _ => None,
    }
}

pub fn spawn(cmd: &Command) -> Result<std::process::Child> {
    if !cfg!(windows) {
        bail!("Launching the PC game is only supported on Windows.");
    }
    tracing::info!("launch: {} {:?}", cmd.program.display(), cmd.args);
    let child = crate::core::process::command(&cmd.program)
        .args(&cmd.args)
        .current_dir(&cmd.cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOBBY: &str = "0f5c1a2b-3c4d-5e6f-7a8b-9c0d1e2f3a4b";

    #[test]
    fn splits_like_a_command_line() {
        assert_eq!(split_args(r#"-a  "b c" d"" "#), ["-a", "b c", "d"]);
        assert!(split_args("  ").is_empty());
        assert_eq!(split_args(r#""""#), [""]);
    }

    #[test]
    fn parses_lobby_ids() {
        let up = LOBBY.to_ascii_uppercase();
        assert_eq!(lobby_uuid(LOBBY).as_deref(), Some(up.as_str()));
        assert_eq!(
            lobby_uuid(&format!("spark://c/{LOBBY}")).as_deref(),
            Some(up.as_str())
        );
        assert_eq!(
            lobby_uuid(&format!("https://echo.taxi/spark://j/{LOBBY}")).as_deref(),
            Some(up.as_str())
        );
        assert_eq!(
            lobby_uuid(&format!("{LOBBY}.nakama2_us-east")).as_deref(),
            Some(up.as_str())
        );
        assert_eq!(lobby_uuid("not-a-lobby"), None);
    }

    #[test]
    fn argv_per_runtime() {
        let exe = Path::new("C:/E/ready-at-dawn-echo-arena/bin/win10/echovr.exe");
        let mut p = LaunchProfile {
            extra_args: "-foo".into(),
            ..Default::default()
        };
        let c = build(&p, exe, None, Some(LOBBY)).unwrap();
        assert_eq!(c.program, exe);
        assert_eq!(c.args, ["-lobbyid", LOBBY, "-foo"]);
        assert_eq!(c.cwd, Path::new("C:/E/ready-at-dawn-echo-arena/bin/win10"));

        p.runtime = Runtime::Flat;
        p.spectator = true;
        p.windowed = true;
        assert_eq!(
            build(&p, exe, None, None).unwrap().args,
            ["-noovr", "-spectatorstream", "-windowed", "-foo"]
        );

        p = LaunchProfile {
            runtime: Runtime::Revive,
            ..Default::default()
        };
        assert!(build(&p, exe, None, None).is_err());
        let c = build(&p, exe, Some("C:/Program Files/Revive"), None).unwrap();
        assert_eq!(
            c.program,
            Path::new("C:/Program Files/Revive/ReviveInjector.exe")
        );
        assert_eq!(c.args[0], exe.to_string_lossy());
        assert_eq!(
            c.args[c.args.len() - 2..],
            ["/app", "ready-at-dawn-echo-arena"]
        );
    }
}
