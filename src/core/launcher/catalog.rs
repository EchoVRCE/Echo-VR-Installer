//! The versions catalogue (`versions.json` on files.echovr.de).
//!
//! ```json
//! { "schema": 1,
//!   "versions": [
//!     { "id": "pc-34.4.631547.1", "name": "Echo VR 34.4 (PC)", "channel": "stable",
//!       "platform": "pc", "url": "ready-at-dawn-echo-arena.zip", "size": 4270000000,
//!       "sha256": null, "update_manifest": "https://files.echovr.de/updates/update.manifest",
//!       "notes": "The last official build, with community patches." } ] }
//! ```
//!
//! `url` is either relative -- served by the fastest download mirror -- or an absolute
//! https URL on a trusted host. Everything is validated before it is used for downloads
//! or folder names.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::core::{pc_update, quest_update};

pub const CATALOG_URL: &str = "https://files.echovr.de/launcher/versions.json";
const TRUSTED_HOSTS: [&str; 2] = ["files.echovr.de", "evr.echo.taxi"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    #[default]
    Pc,
    Quest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct VersionEntry {
    pub id: String,
    pub name: String,
    pub channel: String,
    pub platform: Platform,
    /// PC: the game zip. Quest: the APK.
    pub url: String,
    /// Quest: `_data.zip`.
    pub data_url: Option<String>,
    pub size: Option<u64>,
    pub sha256: Option<String>,
    pub update_manifest: Option<String>,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Catalog {
    pub schema: u32,
    pub versions: Vec<VersionEntry>,
    /// Not in the file: true when the built-in fallback is shown.
    #[serde(skip)]
    pub builtin: bool,
}

/// Ids become folder names: lowercase letters, digits, `.`, `-`, `_`.
pub fn is_safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
}

/// A download reference: relative to the mirror (safe path), or https on a trusted host.
pub fn is_safe_url(url: &str) -> bool {
    if !url.contains("://") {
        return crate::core::manifest::is_safe_path(url);
    }
    url::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https" && u.host_str().is_some_and(|h| TRUSTED_HOSTS.contains(&h))
    })
}

impl VersionEntry {
    /// Whether `url` is served by the mirrors (relative) rather than absolute.
    pub fn uses_mirror(&self) -> bool {
        !self.url.contains("://")
    }

    fn validate(&self) -> Result<()> {
        if !is_safe_id(&self.id) {
            bail!("invalid version id {:?}", self.id);
        }
        if !is_safe_url(&self.url) {
            bail!("untrusted download for {}: {}", self.id, self.url);
        }
        if let Some(d) = &self.data_url {
            if !is_safe_url(d) {
                bail!("untrusted data download for {}: {d}", self.id);
            }
        }
        if let Some(m) = &self.update_manifest {
            if !(m.contains("://") && is_safe_url(m)) {
                bail!("untrusted update manifest for {}: {m}", self.id);
            }
        }
        if let Some(h) = &self.sha256 {
            if h.len() != 64 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
                bail!("invalid sha256 for {}", self.id);
            }
        }
        Ok(())
    }
}

impl Catalog {
    pub fn parse(text: &str) -> Result<Catalog> {
        let c: Catalog = serde_json::from_str(text)?;
        let mut seen = std::collections::HashSet::new();
        for v in &c.versions {
            v.validate()?;
            if !seen.insert(&v.id) {
                bail!("duplicate version id {}", v.id);
            }
        }
        Ok(c)
    }

    pub fn fetch() -> Result<Catalog> {
        Catalog::parse(&crate::core::http::get_text(CATALOG_URL)?)
    }

    /// What the installer has always offered, for when the catalogue is unreachable or
    /// not published yet.
    pub fn builtin() -> Catalog {
        Catalog {
            schema: 1,
            builtin: true,
            versions: vec![
                VersionEntry {
                    id: "pc-latest".into(),
                    name: "Echo VR (PC, latest)".into(),
                    channel: "stable".into(),
                    platform: Platform::Pc,
                    url: "ready-at-dawn-echo-arena.zip".into(),
                    update_manifest: Some(pc_update::PC_MANIFEST_URL.into()),
                    notes: "The current PC client with community updates.".into(),
                    ..Default::default()
                },
                VersionEntry {
                    id: "quest-latest".into(),
                    name: "Echo VR (Quest, latest)".into(),
                    channel: "stable".into(),
                    platform: Platform::Quest,
                    url: "echo_quest_latest.apk".into(),
                    data_url: Some("_data.zip".into()),
                    update_manifest: Some(quest_update::QUEST_MANIFEST_URL.into()),
                    notes: "Installed with the Quest wizard.".into(),
                    ..Default::default()
                },
            ],
        }
    }

    /// The published catalogue, or the built-in one.
    pub fn load() -> Catalog {
        Catalog::fetch().unwrap_or_else(|e| {
            tracing::info!("versions catalogue unavailable ({e:#}); using the built-in list");
            Catalog::builtin()
        })
    }

    pub fn pc(&self) -> impl Iterator<Item = &VersionEntry> {
        self.versions.iter().filter(|v| v.platform == Platform::Pc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{"schema":1,"versions":[
      {"id":"pc-34.4","name":"Echo 34.4","platform":"pc","url":"ready-at-dawn-echo-arena.zip",
       "update_manifest":"https://files.echovr.de/updates/update.manifest"},
      {"id":"pc-beta","name":"Beta","platform":"pc","url":"https://evr.echo.taxi/beta.zip",
       "sha256":"0a7fa5f9cfc173013e152a75fac2ded7ca4f66b8d8530f598c0c2530b5cf0973"}]}"#;

    #[test]
    fn parses_and_classifies() {
        let c = Catalog::parse(GOOD).unwrap();
        assert_eq!(c.versions.len(), 2);
        assert!(c.versions[0].uses_mirror());
        assert!(!c.versions[1].uses_mirror());
        assert_eq!(c.pc().count(), 2);
    }

    #[test]
    fn rejects_untrusted_entries() {
        for bad in [
            r#"{"versions":[{"id":"../x","url":"a.zip"}]}"#,
            r#"{"versions":[{"id":"Upper","url":"a.zip"}]}"#,
            r#"{"versions":[{"id":"a","url":"https://evil.example/a.zip"}]}"#,
            r#"{"versions":[{"id":"a","url":"http://files.echovr.de/a.zip"}]}"#,
            r#"{"versions":[{"id":"a","url":"../a.zip"}]}"#,
            r#"{"versions":[{"id":"a","url":"a.zip","update_manifest":"update.manifest"}]}"#,
            r#"{"versions":[{"id":"a","url":"a.zip","sha256":"xyz"}]}"#,
            r#"{"versions":[{"id":"a","url":"a.zip"},{"id":"a","url":"b.zip"}]}"#,
        ] {
            assert!(Catalog::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn builtin_is_valid() {
        let b = Catalog::builtin();
        for v in &b.versions {
            v.validate().unwrap();
        }
        assert!(b.builtin);
    }
}
