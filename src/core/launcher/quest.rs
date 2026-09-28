//! Echo VR on the Quest: which version is installed, and starting/stopping it over adb.

use anyhow::{bail, Result};

use crate::core::adb::{self, PACKAGE};
use crate::core::error::UiError;
use crate::core::quest_install;
use crate::core::quest_update::{self, Marker};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestInfo {
    pub device: Option<String>,
    pub installed: bool,
    /// From the installer's on-device marker, when present.
    pub marker: Option<Marker>,
}

impl QuestInfo {
    pub fn version_label(&self) -> String {
        match (&self.installed, &self.marker) {
            (false, _) => "Echo VR is not installed on this Quest".into(),
            (true, Some(m)) => {
                let base = m.base_apk.clone().unwrap_or_else(|| "unknown build".into());
                if m.patched {
                    format!("{base} (patched)")
                } else {
                    base
                }
            }
            (true, None) => "Installed (version unknown)".into(),
        }
    }
}

fn ready() -> Result<()> {
    adb::bundle::binary()?;
    match quest_install::status_error(adb::probe().status) {
        None => Ok(()),
        Some(e) => Err(e.into()),
    }
}

pub fn info() -> Result<QuestInfo> {
    ready()?;
    let device = adb::target_device().map(|d| d.label());
    let installed = quest_update::installed_apk_path().is_some();
    let marker = if installed {
        quest_update::read_marker()
    } else {
        None
    };
    Ok(QuestInfo {
        device,
        installed,
        marker,
    })
}

/// Pure: the launchable component from `cmd package resolve-activity --brief <pkg>`,
/// whose last line is `pkg/.Activity` (or `No activity found`).
pub fn parse_activity(output: &str) -> Option<String> {
    output
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| l.starts_with(PACKAGE) && l.contains('/'))
        .map(str::to_string)
}

pub fn launch() -> Result<()> {
    ready()?;
    let out = adb::shell(&format!(
        "cmd package resolve-activity --brief -c android.intent.category.LAUNCHER {PACKAGE}"
    ));
    let Some(component) = parse_activity(&out.output) else {
        return Err(UiError::new(
            "Echo VR not installed",
            "Echo VR is not installed on your Quest.",
        )
        .into());
    };
    let r = adb::exec(&["shell", "am", "start", "-n", &component]);
    if !r.success() || r.output.contains("Error") {
        bail!("The Quest refused to start Echo VR:\n{}", r.output.trim());
    }
    Ok(())
}

pub fn stop() -> Result<()> {
    ready()?;
    adb::exec(&["shell", "am", "force-stop", PACKAGE]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_parsing() {
        let out = "priority=0 preferredOrder=0 match=0x108000 specificIndex=-1 isDefault=true\ncom.readyatdawn.r15/com.unity3d.player.UnityPlayerActivity\n";
        assert_eq!(
            parse_activity(out).as_deref(),
            Some("com.readyatdawn.r15/com.unity3d.player.UnityPlayerActivity")
        );
        assert_eq!(parse_activity("No activity found"), None);
    }

    #[test]
    fn version_label() {
        let mut i = QuestInfo {
            device: None,
            installed: false,
            marker: None,
        };
        assert!(i.version_label().contains("not installed"));
        i.installed = true;
        assert_eq!(i.version_label(), "Installed (version unknown)");
        i.marker = Some(Marker {
            base_apk: Some("echo_quest_27-08-2026.001.apk".into()),
            patched: true,
            ..Default::default()
        });
        assert_eq!(i.version_label(), "echo_quest_27-08-2026.001.apk (patched)");
    }
}
