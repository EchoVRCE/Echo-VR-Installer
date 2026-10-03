//! Mods: EchoLoader 2 and its plugins in a live PC version's `bin/win10` (the contract is
//! EchoVR_Mod_Loader's `docs/formats.md`).
//!
//! The loader is the game's crash reporter, `BugSplat64.dll`; it loads the plugins in
//! `plugins/` that `echoloader.json` names. The community update owns that file, the
//! plugins it ships and `asset_patches/manifest.json` (Update puts them back, Verify
//! reports a change), so the launcher never edits them. It writes only:
//! - `echoloader.local.json`, its overlay: mods on or off, a plugin on or off, a plugin's
//!   arguments, and the plugins it added (each with its checksum, which the loader
//!   checks);
//! - `asset_patches/manifest.local.json`: asset patches on or off;
//! - the plugin files it installed itself (from the mods catalogue, or from disk).
//!
//! The loader writes `plugin_logs/loader-status.json` at every start: what it loaded, and
//! why not.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::catalog;
use super::store::InstalledVersion;
use super::versions::Step;
use crate::core::{download, paths};

pub const CATALOG_URL: &str = "https://files.echovr.de/launcher/mods.json";
/// The loader's slot: the crash reporter the game imports.
pub const SLOT: &str = "BugSplat64.dll";
/// The game's own crash reporter (the live build's, from its file manifest).
const STOCK_SLOT_SHA256: &str = "618e34d3bfde69846c0202d42951366bff5e03cd2885b6825f55e09fb2cf160b";
/// EchoLoader 1 was the game's `dbgcore.dll`.
const LEGACY: &str = "dbgcore.dll";
pub const CONFIG: &str = "echoloader.json";
pub const OVERLAY: &str = "echoloader.local.json";
pub const STATUS: &str = "loader-status.json";
const ASSETS: &str = "asset_patches/manifest.json";
const ASSETS_OVERLAY: &str = "asset_patches/manifest.local.json";
/// The plugin whose patches `asset_patches` lists.
pub const ASSET_PLUGIN: &str = "NvrAssetPatches.dll";
/// What the loader carries in its read-only data: `ECHOLOADER_ID:<version>:<build>`.
const MARKER: &[u8] = b"ECHOLOADER_ID:";

/// What sits in the loader's slot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Loader {
    /// The game's own crash reporter (or nothing): no mods.
    #[default]
    None,
    /// EchoLoader 1, the game's `dbgcore.dll`: it can't take the launcher's overlay.
    Legacy,
    /// EchoLoader 2; `pinned`: the locked build (its plugins are compiled in).
    Current { version: String, pinned: bool },
    /// A DLL the launcher doesn't know (another runtime).
    Unknown,
}

impl Loader {
    /// Whether the launcher can change the mods (EchoLoader 2, not locked).
    pub fn editable(&self) -> bool {
        matches!(self, Loader::Current { pinned: false, .. })
    }
}

/// Pure: the loader `bytes` (the slot's file) are, and whether `legacy` (the game's
/// `dbgcore.dll`) is EchoLoader 1.
pub fn classify(slot: Option<&[u8]>, slot_sha: Option<&str>, legacy: Option<&[u8]>) -> Loader {
    if let Some(bytes) = slot {
        if let Some(at) = find(bytes, MARKER) {
            let id: String = bytes[at + MARKER.len()..]
                .iter()
                .take(64)
                .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b':' | b'-'))
                .map(|&b| b as char)
                .collect();
            let (version, build) = id.split_once(':').unwrap_or((&id, "open"));
            return Loader::Current {
                version: version.to_string(),
                pinned: build == "pinned",
            };
        }
        if !slot_sha.is_some_and(|s| s.eq_ignore_ascii_case(STOCK_SLOT_SHA256)) {
            return Loader::Unknown;
        }
    }
    if legacy.is_some_and(|b| find(b, b"[EchoLoader]").is_some()) {
        Loader::Legacy
    } else {
        Loader::None
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Whether the game's `dbgcore.dll` in `bin` is EchoLoader 1.
pub fn legacy_in(bin: &Path) -> bool {
    std::fs::read(bin.join(LEGACY)).is_ok_and(|b| find(&b, b"[EchoLoader]").is_some())
}

// ---- echoloader.json (the update's) ----

/// An entry of a `plugins` list: a file name, or an object.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Entry {
    pub file: String,
    pub name: String,
    pub enabled: bool,
    pub args: Map<String, Value>,
    pub sha256: Option<String>,
    /// What the launcher noted on an entry it added (the loader ignores the key).
    pub launcher: Option<Added>,
}

/// Where an entry the launcher added came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Added {
    /// The mods catalogue's id, when installed from it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_id: Option<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub version: String,
    /// Added from disk.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub local: bool,
}

impl Entry {
    fn parse(v: &Value) -> Option<Entry> {
        match v {
            Value::String(f) => Some(Entry {
                file: f.clone(),
                enabled: true,
                ..Default::default()
            }),
            Value::Object(o) => {
                let file = o.get("file")?.as_str()?.to_string();
                Some(Entry {
                    name: str_of(o, "name"),
                    enabled: o.get("enabled").and_then(Value::as_bool).unwrap_or(true),
                    args: o
                        .get("args")
                        .and_then(Value::as_object)
                        .cloned()
                        .unwrap_or_default(),
                    sha256: o.get("sha256").and_then(Value::as_str).map(str::to_string),
                    launcher: o
                        .get("launcher")
                        .and_then(|l| serde_json::from_value(l.clone()).ok()),
                    file,
                })
            }
            _ => None,
        }
    }
}

fn str_of(o: &Map<String, Value>, key: &str) -> String {
    o.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn entries(v: Option<&Value>) -> Option<Vec<Entry>> {
    Some(v?.as_array()?.iter().filter_map(Entry::parse).collect())
}

/// The update's `echoloader.json`, as far as the launcher needs it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Config {
    pub log_path: String,
    pub plugins_dir: String,
    /// The top-level list (`None`: none, the loader takes every DLL in the folder).
    pub plugins: Option<Vec<Entry>>,
    pub profiles: BTreeMap<String, Option<Vec<Entry>>>,
    pub default_profile: Option<String>,
}

impl Config {
    pub fn parse(text: &str) -> Config {
        let v: Value = serde_json::from_str(text).unwrap_or_default();
        let o = v.as_object().cloned().unwrap_or_default();
        let profiles = o
            .get("profiles")
            .and_then(Value::as_object)
            .map(|p| {
                p.iter()
                    .map(|(k, v)| (k.clone(), entries(v.get("plugins"))))
                    .collect()
            })
            .unwrap_or_default();
        let dir = |key: &str, default: &str| {
            Some(str_of(&o, key))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| default.into())
        };
        Config {
            log_path: dir("log_path", "plugin_logs"),
            plugins_dir: dir("plugins_dir", "plugins"),
            plugins: entries(o.get("plugins")),
            profiles,
            default_profile: o
                .get("default_profile")
                .and_then(Value::as_str)
                .map(str::to_string),
        }
    }

    /// The list the loader uses with profile `chosen` (the overlay's), as it picks one:
    /// that profile, else `default_profile`, else one named "default", else the top level.
    /// `None`: no list, every DLL in the folder.
    pub fn active(&self, chosen: Option<&str>) -> Option<Vec<Entry>> {
        let profile = [chosen, self.default_profile.as_deref(), Some("default")]
            .into_iter()
            .flatten()
            .find_map(|p| self.profiles.get(p));
        match profile {
            Some(Some(list)) => Some(list.clone()),
            _ => self.plugins.clone(),
        }
    }

    /// Every file a list of it names (whatever the profile).
    fn names(&self) -> impl Iterator<Item = &str> {
        self.plugins
            .iter()
            .chain(self.profiles.values().flatten())
            .flatten()
            .map(|e| e.file.as_str())
    }
}

// ---- echoloader.local.json (the launcher's) ----

/// The launcher's overlay, kept as JSON so keys this version doesn't know survive.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Overlay(Map<String, Value>);

impl Overlay {
    pub fn parse(text: &str) -> Overlay {
        match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(o)) => Overlay(o),
            _ => Overlay::default(),
        }
    }

    pub fn read(path: &Path) -> Overlay {
        std::fs::read_to_string(path)
            .map(|t| Overlay::parse(&t))
            .unwrap_or_default()
    }

    /// Replaces the file at `path` (atomically).
    pub fn write(&self, path: &Path) -> Result<()> {
        let mut o = self.0.clone();
        o.insert("version".into(), 1.into());
        write_json(path, &Value::Object(o))
    }

    /// Mods are on (not "start without mods").
    pub fn enabled(&self) -> bool {
        self.0
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true)
    }

    pub fn set_enabled(&mut self, on: bool) {
        self.0.insert("enabled".into(), on.into());
    }

    pub fn profile(&self) -> Option<&str> {
        self.0.get("profile").and_then(Value::as_str)
    }

    /// The overrides of `file` (matched as the loader matches, ignoring case).
    fn overrides(&self, file: &str) -> Option<&Map<String, Value>> {
        self.0
            .get("overrides")?
            .as_object()?
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(file))
            .and_then(|(_, v)| v.as_object())
    }

    fn overrides_mut(&mut self, file: &str) -> &mut Map<String, Value> {
        let all = self
            .0
            .entry("overrides")
            .or_insert_with(|| Value::Object(Map::new()));
        if !all.is_object() {
            *all = Value::Object(Map::new());
        }
        let all = all.as_object_mut().expect("an object");
        let key = all
            .keys()
            .find(|k| k.eq_ignore_ascii_case(file))
            .cloned()
            .unwrap_or_else(|| file.to_string());
        let o = all.entry(key).or_insert_with(|| Value::Object(Map::new()));
        if !o.is_object() {
            *o = Value::Object(Map::new());
        }
        o.as_object_mut().expect("an object")
    }

    /// The plugins the launcher added, in order.
    pub fn added(&self) -> Vec<Entry> {
        entries(self.0.get("add")).unwrap_or_default()
    }

    fn added_mut(&mut self, file: &str) -> Option<&mut Map<String, Value>> {
        self.0
            .get_mut("add")?
            .as_array_mut()?
            .iter_mut()
            .filter_map(Value::as_object_mut)
            .find(|o| {
                o.get("file")
                    .and_then(Value::as_str)
                    .is_some_and(|f| f.eq_ignore_ascii_case(file))
            })
    }

    /// Adds `e` (or replaces the entry of its file).
    pub fn add(&mut self, e: &Entry) {
        let mut o = Map::new();
        o.insert("file".into(), e.file.clone().into());
        if !e.name.is_empty() {
            o.insert("name".into(), e.name.clone().into());
        }
        o.insert("enabled".into(), e.enabled.into());
        if !e.args.is_empty() {
            o.insert("args".into(), Value::Object(e.args.clone()));
        }
        if let Some(sha) = &e.sha256 {
            o.insert("sha256".into(), sha.clone().into());
        }
        if let Some(l) = &e.launcher {
            o.insert(
                "launcher".into(),
                serde_json::to_value(l).unwrap_or_default(),
            );
        }
        match self.added_mut(&e.file) {
            Some(existing) => *existing = o,
            None => {
                let list = self
                    .0
                    .entry("add")
                    .or_insert_with(|| Value::Array(Vec::new()));
                if !list.is_array() {
                    *list = Value::Array(Vec::new());
                }
                list.as_array_mut()
                    .expect("an array")
                    .push(Value::Object(o));
            }
        }
    }

    /// Takes `file` out of the added plugins; whether it was there.
    pub fn remove_added(&mut self, file: &str) -> bool {
        let Some(list) = self.0.get_mut("add").and_then(Value::as_array_mut) else {
            return false;
        };
        let before = list.len();
        list.retain(|v| {
            !v.get("file")
                .and_then(Value::as_str)
                .is_some_and(|f| f.eq_ignore_ascii_case(file))
        });
        before != list.len()
    }

    /// Turns `file` on or off: on its entry when the launcher added it, else as an
    /// override.
    pub fn set_plugin_enabled(&mut self, file: &str, on: bool) {
        match self.added_mut(file) {
            Some(o) => {
                o.insert("enabled".into(), on.into());
            }
            None => {
                self.overrides_mut(file).insert("enabled".into(), on.into());
            }
        }
    }

    /// Sets `file`'s arguments (each value a string, as the loader hands them on).
    pub fn set_args(&mut self, file: &str, args: &BTreeMap<String, String>) {
        let args: Map<String, Value> = args
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        match self.added_mut(file) {
            Some(o) => {
                o.insert("args".into(), Value::Object(args));
            }
            None => {
                self.overrides_mut(file)
                    .insert("args".into(), Value::Object(args));
            }
        }
    }

    /// `list` as the loader applies the overlay to it: overrides, then the added
    /// plugins appended (one already listed acts as an override).
    pub fn apply(&self, list: Vec<Entry>) -> Vec<Entry> {
        let mut out: Vec<Entry> = list
            .into_iter()
            .map(|mut e| {
                if let Some(o) = self.overrides(&e.file) {
                    override_entry(&mut e, o);
                }
                e
            })
            .collect();
        for a in self.added() {
            match out
                .iter_mut()
                .find(|e| e.file.eq_ignore_ascii_case(&a.file))
            {
                Some(e) => {
                    e.enabled = a.enabled;
                    if !a.args.is_empty() {
                        e.args = a.args;
                    }
                    if a.sha256.is_some() {
                        e.sha256 = a.sha256;
                    }
                    e.launcher = a.launcher;
                }
                None => out.push(a),
            }
        }
        out
    }
}

fn override_entry(e: &mut Entry, o: &Map<String, Value>) {
    if let Some(on) = o.get("enabled").and_then(Value::as_bool) {
        e.enabled = on;
    }
    if let Some(args) = o.get("args").and_then(Value::as_object) {
        e.args = args.clone();
    }
    if let Some(sha) = o.get("sha256").and_then(Value::as_str) {
        e.sha256 = Some(sha.to_string());
    }
    if let Some(n) = o.get("name").and_then(Value::as_str) {
        e.name = n.to_string();
    }
}

fn write_json(path: &Path, v: &Value) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(v)?)
        .with_context(|| format!("Couldn't write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| {
        let _ = std::fs::remove_file(&tmp);
        format!("Couldn't replace {}", path.display())
    })
}

// ---- loader-status.json (the loader's) ----

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(default)]
pub struct LoaderInfo {
    pub version: String,
    pub build: String,
    pub api: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(default)]
pub struct PluginStatus {
    pub file: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub api: u32,
    pub capabilities: u32,
    pub source: String,
    /// `loaded`, `failed`, `rejected`, `collision`, `pin_mismatch`, `disabled`, `denied`,
    /// `missing`.
    pub status: String,
    pub error: String,
    pub sha256: String,
}

impl PluginStatus {
    pub fn loaded(&self) -> bool {
        self.status == "loaded"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(default)]
pub struct Summary {
    pub loaded: u32,
    pub failed: u32,
    pub skipped: u32,
}

/// What the loader did at the last start.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
#[serde(default)]
pub struct Status {
    pub schema: u32,
    pub loader: LoaderInfo,
    /// UTC, `2026-10-03T18:00:00Z`.
    pub started: String,
    pub profile: String,
    pub mods_enabled: bool,
    pub plugins: Vec<PluginStatus>,
    pub summary: Summary,
}

impl Status {
    pub fn parse(text: &str) -> Option<Status> {
        serde_json::from_str(text).ok()
    }

    fn of(&self, file: &str) -> Option<&PluginStatus> {
        self.plugins
            .iter()
            .find(|p| p.file.eq_ignore_ascii_case(file))
    }
}

// ---- asset patches ----

/// An asset patch the update ships, and whether it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetPatch {
    pub label: String,
    pub enabled: bool,
}

/// Pure: the patches of `manifest` (the update's) with `overlay`'s choices, and whether
/// asset patches are on at all.
pub fn asset_patches(manifest: &str, overlay: &str) -> (bool, Vec<AssetPatch>) {
    let m: Value = serde_json::from_str(manifest).unwrap_or_default();
    let o: Value = serde_json::from_str(overlay).unwrap_or_default();
    let all_on = o.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let patches = m
        .get("patches")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            let label = p.get("label")?.as_str()?.to_string();
            let shipped = p.get("enabled").and_then(Value::as_bool).unwrap_or(true);
            let chosen = o
                .get("patches")
                .and_then(|ps| ps.get(&label))
                .and_then(|c| c.get("enabled"))
                .and_then(Value::as_bool);
            Some(AssetPatch {
                enabled: chosen.unwrap_or(shipped),
                label,
            })
        })
        .collect();
    (all_on, patches)
}

/// Pure: `overlay` (the asset patches' overlay) with `label` turned on or off (`None`:
/// every patch).
fn set_asset(overlay: &str, label: Option<&str>, on: bool) -> Value {
    let mut o = match serde_json::from_str::<Value>(overlay) {
        Ok(Value::Object(o)) => o,
        _ => Map::new(),
    };
    o.insert("version".into(), 1.into());
    match label {
        None => {
            o.insert("enabled".into(), on.into());
        }
        Some(label) => {
            let patches = o
                .entry("patches")
                .or_insert_with(|| Value::Object(Map::new()));
            if !patches.is_object() {
                *patches = Value::Object(Map::new());
            }
            let mut choice = Map::new();
            choice.insert("enabled".into(), on.into());
            patches
                .as_object_mut()
                .expect("an object")
                .insert(label.to_string(), Value::Object(choice));
        }
    }
    Value::Object(o)
}

// ---- the view ----

/// Where a plugin comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// The community update ships it.
    Shipped,
    /// The launcher installed it from the mods catalogue.
    Catalog { id: String, version: String },
    /// Added from disk (or put into the folder by hand).
    Local,
}

/// A plugin as the Mods page shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Plugin {
    pub file: String,
    /// Its own name (what it reported at the last start), else the configured one, else
    /// the file's.
    pub name: String,
    /// Its own version, from the last start.
    pub version: String,
    pub source: Source,
    /// The loader loads it (in its list and turned on).
    pub enabled: bool,
    /// In the loader's list (a DLL in the folder may not be).
    pub listed: bool,
    pub args: Map<String, Value>,
    /// The file is there.
    pub present: bool,
    /// What the loader did with it at the last start.
    pub status: Option<PluginStatus>,
}

impl Plugin {
    /// The launcher put its file there (and may take it away).
    pub fn removable(&self) -> bool {
        matches!(self.source, Source::Catalog { .. })
            || (self.source == Source::Local && self.listed)
    }

    /// Its arguments as text fields: strings as they are, anything else as JSON.
    pub fn arg_strings(&self) -> BTreeMap<String, String> {
        self.args
            .iter()
            .map(|(k, v)| {
                let text = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                (k.clone(), text)
            })
            .collect()
    }
}

/// A version's mods, as the Mods page shows them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModView {
    pub loader: Loader,
    /// Mods on (the overlay's "start without mods" is off).
    pub enabled: bool,
    pub plugins: Vec<Plugin>,
    /// The last start, if the loader ever started.
    pub status: Option<Status>,
    /// Asset patches on at all, and each.
    pub assets_enabled: bool,
    pub asset_patches: Vec<AssetPatch>,
}

/// The folder its plugins are in.
pub fn plugins_dir(v: &InstalledVersion) -> PathBuf {
    let bin = v.bin_dir();
    bin.join(config(&bin).plugins_dir)
}

/// The folder the loader and its plugins log to.
pub fn log_dir(bin: &Path) -> PathBuf {
    bin.join(config(bin).log_path)
}

fn config(bin: &Path) -> Config {
    Config::parse(&std::fs::read_to_string(bin.join(CONFIG)).unwrap_or_default())
}

/// Reads `v`'s mods (file I/O: on a worker).
pub fn read(v: &InstalledVersion) -> ModView {
    let bin = v.bin_dir();
    let slot = std::fs::read(bin.join(SLOT)).ok();
    let slot_sha = slot
        .as_ref()
        .and_then(|b| download::sha256_reader(&mut b.as_slice()).ok());
    let legacy = std::fs::read(bin.join(LEGACY)).ok();
    let loader = classify(slot.as_deref(), slot_sha.as_deref(), legacy.as_deref());
    let cfg = config(&bin);
    let overlay = Overlay::read(&bin.join(OVERLAY));
    let status = std::fs::read_to_string(bin.join(&cfg.log_path).join(STATUS))
        .ok()
        .and_then(|t| Status::parse(&t));
    let files = dlls_in(&bin.join(&cfg.plugins_dir));
    let read = |p: &str| std::fs::read_to_string(bin.join(p)).unwrap_or_default();
    let (assets_enabled, asset_patches) = asset_patches(&read(ASSETS), &read(ASSETS_OVERLAY));
    ModView {
        enabled: overlay.enabled(),
        plugins: plugins(&cfg, &overlay, &files, status.as_ref()),
        loader,
        status,
        assets_enabled,
        asset_patches,
    }
}

/// The DLL file names in `dir`, sorted.
fn dlls_in(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_ascii_lowercase().ends_with(".dll"))
        .collect();
    out.sort_by_key(|n| n.to_ascii_lowercase());
    out
}

/// Pure: the plugins the page lists: the loader's list with the overlay applied (or,
/// without a list, every DLL in the folder), then the folder's other DLLs, off.
pub fn plugins(
    cfg: &Config,
    overlay: &Overlay,
    files: &[String],
    status: Option<&Status>,
) -> Vec<Plugin> {
    let has = |f: &str| files.iter().any(|x| x.eq_ignore_ascii_case(f));
    let shipped = |f: &str| cfg.names().any(|n| n.eq_ignore_ascii_case(f));
    let scan = cfg.active(overlay.profile()).is_none();
    let base = cfg.active(overlay.profile()).unwrap_or_else(|| {
        files
            .iter()
            .filter(|f| {
                !overlay
                    .added()
                    .iter()
                    .any(|a| a.file.eq_ignore_ascii_case(f))
            })
            .map(|f| Entry {
                file: f.clone(),
                enabled: true,
                ..Default::default()
            })
            .collect()
    });
    let mut out: Vec<Plugin> = overlay
        .apply(base)
        .into_iter()
        .map(|e| {
            let source = match &e.launcher {
                Some(Added {
                    catalog_id: Some(id),
                    version,
                    ..
                }) => Source::Catalog {
                    id: id.clone(),
                    version: version.clone(),
                },
                Some(_) => Source::Local,
                None if shipped(&e.file) || scan => Source::Shipped,
                None => Source::Local,
            };
            plugin(e, source, true, has, status)
        })
        .collect();
    for f in files {
        if !out.iter().any(|p| p.file.eq_ignore_ascii_case(f)) {
            let e = Entry {
                file: f.clone(),
                ..Default::default()
            };
            out.push(plugin(e, Source::Local, false, has, status));
        }
    }
    out
}

fn plugin(
    e: Entry,
    source: Source,
    listed: bool,
    has: impl Fn(&str) -> bool,
    status: Option<&Status>,
) -> Plugin {
    let st = status.and_then(|s| s.of(&e.file)).cloned();
    let stem = e
        .file
        .rsplit_once('.')
        .map_or(e.file.as_str(), |(s, _)| s)
        .to_string();
    let name = st
        .as_ref()
        .map(|s| s.name.clone())
        .filter(|n| !n.is_empty())
        .or_else(|| Some(e.name.clone()).filter(|n| !n.is_empty()))
        .unwrap_or(stem);
    Plugin {
        present: has(&e.file),
        name,
        version: st.as_ref().map(|s| s.version.clone()).unwrap_or_default(),
        source,
        enabled: listed && e.enabled,
        listed,
        args: e.args,
        status: st,
        file: e.file,
    }
}

// ---- changes ----

/// Mods on or off for `v` (the overlay's "start without mods").
pub fn set_enabled(v: &InstalledVersion, on: bool) -> Result<()> {
    edit_overlay(v, |o| o.set_enabled(on))
}

/// `file` on or off. A DLL in the folder the loader doesn't list is added (with its
/// checksum) when turned on.
pub fn set_plugin_enabled(v: &InstalledVersion, file: &str, on: bool) -> Result<()> {
    let bin = v.bin_dir();
    let cfg = config(&bin);
    let overlay = Overlay::read(&bin.join(OVERLAY));
    let listed = cfg
        .active(overlay.profile())
        .is_none_or(|l| l.iter().any(|e| e.file.eq_ignore_ascii_case(file)))
        || overlay
            .added()
            .iter()
            .any(|a| a.file.eq_ignore_ascii_case(file));
    if listed || !on {
        return edit_overlay(v, |o| o.set_plugin_enabled(file, on));
    }
    let sha = download::sha256_file(&bin.join(&cfg.plugins_dir).join(file))?;
    edit_overlay(v, |o| {
        o.add(&Entry {
            file: file.to_string(),
            enabled: true,
            sha256: Some(sha),
            launcher: Some(Added {
                local: true,
                ..Default::default()
            }),
            ..Default::default()
        })
    })
}

/// `file`'s arguments.
pub fn set_args(v: &InstalledVersion, file: &str, args: &BTreeMap<String, String>) -> Result<()> {
    edit_overlay(v, |o| o.set_args(file, args))
}

fn edit_overlay(v: &InstalledVersion, f: impl FnOnce(&mut Overlay)) -> Result<()> {
    let path = v.bin_dir().join(OVERLAY);
    let mut o = Overlay::read(&path);
    f(&mut o);
    o.write(&path)
}

/// Asset patch `label` on or off (`None`: all of them).
pub fn set_asset_patch(v: &InstalledVersion, label: Option<&str>, on: bool) -> Result<()> {
    let path = v.bin_dir().join(ASSETS_OVERLAY);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    write_json(&path, &set_asset(&text, label, on))
}

/// Pure: whether `name` is a plugin file name the launcher takes (no folders).
pub fn is_plugin_file(name: &str) -> bool {
    name.len() <= 64
        && !name.starts_with('.')
        && name.to_ascii_lowercase().ends_with(".dll")
        && name.len() > 4
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

/// Puts `src` into `v`'s plugins folder as `file` (staged beside it, then renamed).
fn place(v: &InstalledVersion, src: &Path, file: &str) -> Result<()> {
    let dir = plugins_dir(v);
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let target = dir.join(file);
    let staged = dir.join(format!(".{file}.download"));
    std::fs::copy(src, &staged).with_context(|| format!("Couldn't copy into {}", dir.display()))?;
    std::fs::rename(&staged, &target).map_err(|e| {
        let _ = std::fs::remove_file(&staged);
        if crate::core::pc_update::is_locked(&e) {
            anyhow::anyhow!(
                "Couldn't replace {file}: Echo VR uses it. Close Echo VR and try again."
            )
        } else {
            anyhow::Error::from(e).context(format!("replace {}", target.display()))
        }
    })
}

/// A file the update ships (or one in the folder the launcher didn't put there) can't be
/// replaced by one it installs.
fn check_free(v: &InstalledVersion, file: &str) -> Result<()> {
    let view = read(v);
    if let Some(p) = view
        .plugins
        .iter()
        .find(|p| p.file.eq_ignore_ascii_case(file) && p.present && !p.removable())
    {
        bail!(
            "{} already has a plugin named {file} ({}). Remove it first, or rename the file.",
            v.name,
            p.name
        );
    }
    Ok(())
}

/// Installs mod `e` into `v`: downloaded, checked against its checksum, put into the
/// plugins folder and added (on) with that checksum, which the loader checks too.
pub fn install(
    v: &InstalledVersion,
    e: &ModEntry,
    cancel: &AtomicBool,
    on: &mut dyn FnMut(Step),
) -> Result<()> {
    let Some(sha) = e.sha256.as_deref().filter(|_| e.downloadable()) else {
        bail!(
            "{} can't be downloaded: it comes with the community update.",
            e.name
        );
    };
    check_free(v, &e.file)?;
    on(Step::Status(format!("Downloading {}...", e.name)));
    let file = download::fetch_pinned(
        &e.url,
        &paths::downloads_dir().join("mods"),
        &e.file,
        sha,
        cancel,
        &mut |p| {
            if let download::Progress::Percent(p) = p {
                on(Step::Percent(p));
            }
        },
    )?;
    on(Step::Status(format!("Installing {}...", e.name)));
    place(v, &file, &e.file)?;
    let _ = std::fs::remove_file(&file);
    edit_overlay(v, |o| {
        o.add(&Entry {
            file: e.file.clone(),
            name: e.name.clone(),
            enabled: true,
            args: e.args.clone(),
            sha256: Some(sha.to_ascii_lowercase()),
            launcher: Some(Added {
                catalog_id: Some(e.id.clone()),
                version: e.version.clone(),
                local: false,
            }),
        })
    })
}

/// Adds the DLL at `src` to `v` (copied into its plugins folder, on, with its checksum).
/// Returns the file name.
pub fn add_local(v: &InstalledVersion, src: &Path) -> Result<String> {
    let file = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !is_plugin_file(&file) {
        bail!("{file:?} isn't a plugin file name the loader takes: letters, digits, '.', '-' and '_', ending in .dll.");
    }
    if file.eq_ignore_ascii_case(SLOT) {
        bail!("{SLOT} is the mod loader itself, not a plugin.");
    }
    check_free(v, &file)?;
    let sha = download::sha256_file(src)?;
    place(v, src, &file)?;
    edit_overlay(v, |o| {
        o.add(&Entry {
            file: file.clone(),
            enabled: true,
            sha256: Some(sha),
            launcher: Some(Added {
                local: true,
                ..Default::default()
            }),
            ..Default::default()
        })
    })?;
    Ok(file)
}

/// Takes plugin `file` out of `v`: its entry, and its file (only one the launcher put
/// there).
pub fn remove(v: &InstalledVersion, file: &str) -> Result<()> {
    let view = read(v);
    let Some(p) = view
        .plugins
        .iter()
        .find(|p| p.file.eq_ignore_ascii_case(file))
    else {
        bail!("{file} isn't a plugin of {}.", v.name);
    };
    if !p.removable() {
        bail!("{file} comes with the community update: turn it off instead.");
    }
    let path = plugins_dir(v).join(&p.file);
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) if crate::core::pc_update::is_locked(&e) => {
            bail!("Couldn't delete {file}: Echo VR uses it. Close Echo VR and try again.")
        }
        Err(e) => return Err(anyhow::Error::from(e).context(format!("delete {}", path.display()))),
    }
    edit_overlay(v, |o| {
        o.remove_added(file);
    })
}

// ---- the mods catalogue (mods.json on files.echovr.de) ----

/// A mod in the catalogue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ModEntry {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub author: String,
    pub version: String,
    /// The plugin's file name in `plugins/`.
    pub file: String,
    /// Relative to the download mirrors, or https on a trusted host. Empty for a shipped
    /// one.
    pub url: String,
    pub sha256: Option<String>,
    pub size: Option<u64>,
    /// The plugin ABI version it was built for.
    pub api: Option<u32>,
    /// What it does: observes-only, cosmetic, alters-gameplay, alters-rules, network,
    /// hooks-engine.
    pub capabilities: Vec<String>,
    /// Its default arguments.
    pub args: Map<String, Value>,
    pub homepage: String,
    /// It comes with the community update: shown, never downloaded.
    pub shipped: bool,
}

impl ModEntry {
    pub fn downloadable(&self) -> bool {
        !self.shipped && !self.url.is_empty()
    }

    fn validate(&self) -> Result<()> {
        if !catalog::is_safe_id(&self.id) {
            bail!("invalid mod id {:?}", self.id);
        }
        if !is_plugin_file(&self.file) || self.file.eq_ignore_ascii_case(SLOT) {
            bail!("invalid plugin file for {}: {:?}", self.id, self.file);
        }
        if !self.shipped {
            if self.url.is_empty() || !catalog::is_safe_url(&self.url) {
                bail!("untrusted download for {}: {}", self.id, self.url);
            }
            let sha = self.sha256.as_deref().unwrap_or_default();
            if sha.len() != 64 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
                bail!("{} has no valid sha256", self.id);
            }
        }
        if !self.homepage.is_empty() && !self.homepage.starts_with("https://") {
            bail!("invalid homepage for {}", self.id);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModCatalog {
    pub mods: Vec<ModEntry>,
    /// The built-in list is shown (the published one couldn't be fetched).
    pub builtin: bool,
}

impl ModCatalog {
    /// The catalogue in `text`; entries that fail validation (or repeat an id) are left
    /// out and logged.
    pub fn parse(text: &str) -> Result<ModCatalog> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            mods: Vec<Value>,
        }
        let raw: Raw = serde_json::from_str(text)?;
        let mut seen = std::collections::HashSet::new();
        let mut mods = Vec::new();
        for (i, value) in raw.mods.into_iter().enumerate() {
            let entry = serde_json::from_value::<ModEntry>(value)
                .map_err(anyhow::Error::from)
                .and_then(|m| m.validate().map(|()| m));
            match entry {
                Ok(m) if seen.insert(m.id.clone()) => mods.push(m),
                Ok(m) => tracing::warn!("mods catalogue: duplicate id {}, left out", m.id),
                Err(e) => tracing::warn!("mods catalogue: entry {i} left out: {e:#}"),
            }
        }
        Ok(ModCatalog {
            mods,
            builtin: false,
        })
    }

    /// The draft in this repo, for when the published one can't be had.
    pub fn builtin() -> ModCatalog {
        let mut c =
            ModCatalog::parse(include_str!("../../../docs/launcher/mods.json")).unwrap_or_default();
        c.builtin = true;
        c
    }

    /// The published catalogue, or the built-in one.
    pub fn load() -> ModCatalog {
        crate::core::http::get_text(CATALOG_URL)
            .and_then(|t| ModCatalog::parse(&t))
            .unwrap_or_else(|e| {
                tracing::info!("mods catalogue unavailable ({e:#}); using the built-in one");
                ModCatalog::builtin()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIVE: &str = r#"{
        "log_path": "plugin_logs",
        "plugins_dir": "plugins",
        "plugins": [
            { "file": "dbgcore.dll" },
            { "file": "NvrAssetPatches.dll", "args": { "logging": "normal" } }
        ]
    }"#;

    fn names(ps: &[Plugin]) -> Vec<(&str, bool)> {
        ps.iter().map(|p| (p.file.as_str(), p.enabled)).collect()
    }

    #[test]
    fn tells_the_loaders_apart() {
        let ours = b"MZ....ECHOLOADER_ID:2.0.0:open\0rest".as_slice();
        assert_eq!(
            classify(Some(ours), None, None),
            Loader::Current {
                version: "2.0.0".into(),
                pinned: false
            }
        );
        let pinned = b"xxECHOLOADER_ID:2.1.3:pinned\0".as_slice();
        assert_eq!(
            classify(Some(pinned), None, None),
            Loader::Current {
                version: "2.1.3".into(),
                pinned: true
            }
        );
        let stock = Some(STOCK_SLOT_SHA256);
        let legacy = b"..[EchoLoader] === Plugin Loader ===".as_slice();
        assert_eq!(classify(Some(b"MZ"), stock, Some(legacy)), Loader::Legacy);
        assert_eq!(classify(Some(b"MZ"), stock, Some(b"MZ")), Loader::None);
        assert_eq!(classify(None, None, None), Loader::None);
        assert_eq!(
            classify(Some(b"MZ nevr"), Some("00"), None),
            Loader::Unknown
        );
        assert!(!Loader::Current {
            version: "2".into(),
            pinned: true
        }
        .editable());
    }

    #[test]
    fn reads_the_updates_config() {
        let c = Config::parse(LIVE);
        assert_eq!(c.log_path, "plugin_logs");
        let list = c.active(None).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].args["logging"], "normal");
        // Flat entries, profiles and the profile choice.
        let c = Config::parse(
            r#"{"plugins":["a.dll","b.dll"],"default_profile":"comp",
                "profiles":{"comp":{"plugins":[{"file":"a.dll","sha256":"ab"}]},
                            "bare":{}}}"#,
        );
        let files =
            |l: Option<Vec<Entry>>| l.unwrap().into_iter().map(|e| e.file).collect::<Vec<_>>();
        assert_eq!(files(c.active(None)), ["a.dll"]);
        assert_eq!(files(c.active(Some("nope"))), ["a.dll"]);
        // A profile without a list: the top level's.
        assert_eq!(files(c.active(Some("bare"))), ["a.dll", "b.dll"]);
        assert!(Config::parse("{}").active(None).is_none());
        assert_eq!(Config::parse("not json").plugins_dir, "plugins");
    }

    #[test]
    fn applies_the_overlay_as_the_loader_does() {
        let cfg = Config::parse(LIVE);
        let mut o = Overlay::default();
        o.set_plugin_enabled("NVRASSETPATCHES.dll", false);
        o.add(&Entry {
            file: "Mine.dll".into(),
            enabled: true,
            sha256: Some("cd".repeat(32)),
            launcher: Some(Added {
                local: true,
                ..Default::default()
            }),
            ..Default::default()
        });
        let files: Vec<String> = [
            "dbgcore.dll",
            "NvrAssetPatches.dll",
            "Mine.dll",
            "Stray.dll",
        ]
        .map(String::from)
        .into();
        let ps = plugins(&cfg, &o, &files, None);
        assert_eq!(
            names(&ps),
            [
                ("dbgcore.dll", true),
                ("NvrAssetPatches.dll", false),
                ("Mine.dll", true),
                ("Stray.dll", false)
            ]
        );
        let src: Vec<_> = ps.iter().map(|p| p.source.clone()).collect();
        assert_eq!(
            src,
            [
                Source::Shipped,
                Source::Shipped,
                Source::Local,
                Source::Local
            ]
        );
        assert!(ps[2].removable() && !ps[0].removable() && !ps[3].removable());
        assert!(ps[3].present && !ps[3].listed);

        // An added entry naming a listed file acts as an override.
        let mut o2 = Overlay::default();
        o2.add(&Entry {
            file: "dbgcore.dll".into(),
            enabled: false,
            ..Default::default()
        });
        assert!(!plugins(&cfg, &o2, &files, None)[0].enabled);

        // Arguments replace the shipped ones; on/off on an added entry stays on it.
        o.set_args(
            "NvrAssetPatches.dll",
            &BTreeMap::from([("logging".into(), "verbose".into())]),
        );
        o.set_plugin_enabled("mine.dll", false);
        let ps = plugins(&cfg, &o, &files, None);
        assert_eq!(ps[1].args["logging"], "verbose");
        assert!(!ps[2].enabled);
        assert!(o.overrides("mine.dll").is_none());
        assert!(o.remove_added("MINE.DLL"));
        assert!(!o.remove_added("mine.dll"));
    }

    #[test]
    fn without_a_list_every_dll_is_shipped() {
        let cfg = Config::parse(r#"{"log_path":"logs"}"#);
        let files = vec!["a.dll".to_string()];
        let ps = plugins(&cfg, &Overlay::default(), &files, None);
        assert_eq!(names(&ps), [("a.dll", true)]);
        assert_eq!(ps[0].source, Source::Shipped);
    }

    #[test]
    fn overlay_round_trips_and_keeps_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(OVERLAY);
        std::fs::write(
            &path,
            r#"{"version":1,"future":{"x":1},"overrides":{"A.dll":{"required":true}}}"#,
        )
        .unwrap();
        let mut o = Overlay::read(&path);
        assert!(o.enabled());
        o.set_enabled(false);
        o.set_plugin_enabled("a.dll", false);
        o.add(&Entry {
            file: "B.dll".into(),
            enabled: true,
            launcher: Some(Added {
                catalog_id: Some("b".into()),
                version: "1.0".into(),
                local: false,
            }),
            ..Default::default()
        });
        o.write(&path).unwrap();
        let back = Overlay::read(&path);
        assert!(!back.enabled());
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(raw["future"]["x"], 1);
        assert_eq!(raw["overrides"]["A.dll"]["required"], true);
        assert_eq!(raw["overrides"]["A.dll"]["enabled"], false);
        assert_eq!(raw["add"][0]["launcher"]["catalog_id"], "b");
        assert!(raw["add"][0]["launcher"].get("local").is_none());
        assert_eq!(back.added()[0].launcher.as_ref().unwrap().version, "1.0");
        assert!(!dir.path().join("echoloader.local.json.tmp").exists());
        // Garbage reads as empty.
        assert_eq!(Overlay::parse("[1,2]"), Overlay::default());
    }

    #[test]
    fn reads_the_loaders_status() {
        let s = Status::parse(
            r#"{"schema":1,"loader":{"version":"2.0.0","build":"open","api":5},
                "started":"2026-10-03T18:00:00Z","pid":1,"profile":"default","mods_enabled":true,
                "plugins":[{"file":"NvrAssetPatches.dll","name":"asset_patches","version":"1.1.0",
                  "api":5,"capabilities":34,"source":"config","status":"loaded","sha256":"ab"},
                  {"file":"x.dll","status":"pin_mismatch","error":"checksum"}],
                "summary":{"loaded":1,"failed":0,"skipped":1}}"#,
        )
        .unwrap();
        assert_eq!(s.summary.skipped, 1);
        let ps = plugins(
            &Config::parse(LIVE),
            &Overlay::default(),
            &["NvrAssetPatches.dll".into()],
            Some(&s),
        );
        assert_eq!(ps[1].name, "asset_patches");
        assert_eq!(ps[1].version, "1.1.0");
        assert!(ps[1].status.as_ref().unwrap().loaded());
        assert!(!ps[0].present && ps[0].status.is_none());
        assert!(Status::parse("nope").is_none());
    }

    #[test]
    fn asset_patches_take_the_launchers_choices() {
        let manifest = r#"{"version":"1.0","patches":[
            {"label":"netgun_base","enabled":true},{"label":"poster_a_tex","enabled":true},
            {"label":"off_by_update","enabled":false},{"enabled":true}]}"#;
        let (on, ps) = asset_patches(manifest, "");
        assert!(on);
        assert_eq!(ps.len(), 3);
        assert!(!ps[2].enabled);
        let o = set_asset("", Some("poster_a_tex"), false);
        let o = set_asset(&o.to_string(), None, false);
        let (on, ps) = asset_patches(manifest, &o.to_string());
        assert!(!on);
        assert!(ps[0].enabled && !ps[1].enabled);
        assert_eq!(o["version"], 1);
    }

    #[test]
    fn installs_adds_and_removes_plugins() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("pc");
        let bin = root.join(paths::ARENA_DIR).join("bin/win10");
        std::fs::create_dir_all(bin.join("plugins")).unwrap();
        std::fs::write(bin.join("echovr.exe"), "MZ").unwrap();
        std::fs::write(bin.join(CONFIG), LIVE).unwrap();
        std::fs::write(bin.join("plugins/NvrAssetPatches.dll"), "MZ shipped").unwrap();
        let v = InstalledVersion {
            id: "pc-latest".into(),
            name: "Echo VR".into(),
            root: paths::normalize(&root.to_string_lossy()),
            ..Default::default()
        };
        let src = dir.path().join("MyMod.dll");
        std::fs::write(&src, "MZ mine").unwrap();
        assert_eq!(add_local(&v, &src).unwrap(), "MyMod.dll");
        let view = read(&v);
        let mine = view.plugins.iter().find(|p| p.file == "MyMod.dll").unwrap();
        assert!(mine.enabled && mine.present && mine.removable());
        let o = Overlay::read(&bin.join(OVERLAY));
        assert_eq!(
            o.added()[0].sha256.as_deref(),
            Some(download::sha256_file(&src).unwrap().as_str())
        );
        // The update's plugin can't be replaced or removed.
        let shadow = dir.path().join("NvrAssetPatches.dll");
        std::fs::write(&shadow, "MZ other").unwrap();
        assert!(add_local(&v, &shadow).is_err());
        assert!(remove(&v, "NvrAssetPatches.dll").is_err());
        assert!(add_local(&v, &dir.path().join("BugSplat64.dll")).is_err());

        set_plugin_enabled(&v, "NvrAssetPatches.dll", false).unwrap();
        set_enabled(&v, false).unwrap();
        let view = read(&v);
        assert!(!view.enabled);
        assert!(!view.plugins[1].enabled);

        remove(&v, "MyMod.dll").unwrap();
        assert!(!bin.join("plugins/MyMod.dll").exists());
        assert!(read(&v).plugins.iter().all(|p| p.file != "MyMod.dll"));

        // A DLL dropped into the folder by hand is added when turned on.
        std::fs::write(bin.join("plugins/Hand.dll"), "MZ hand").unwrap();
        assert!(!read(&v)
            .plugins
            .iter()
            .any(|p| p.file == "Hand.dll" && p.enabled));
        set_plugin_enabled(&v, "Hand.dll", true).unwrap();
        let hand = read(&v)
            .plugins
            .into_iter()
            .find(|p| p.file == "Hand.dll")
            .unwrap();
        assert!(hand.enabled && hand.removable());
    }

    #[test]
    fn catalogue_takes_only_safe_entries() {
        let sha = "ab".repeat(32);
        let c = ModCatalog::parse(&format!(
            r#"{{"schema":1,"mods":[
              {{"id":"good","name":"Good","file":"Good.dll","url":"mods/Good.dll","sha256":"{sha}"}},
              {{"id":"shipped","file":"NvrAssetPatches.dll","shipped":true}},
              {{"id":"nosha","file":"a.dll","url":"mods/a.dll"}},
              {{"id":"evil","file":"e.dll","url":"https://evil.example/e.dll","sha256":"{sha}"}},
              {{"id":"path","file":"../x.dll","shipped":true}},
              {{"id":"slot","file":"BugSplat64.dll","shipped":true}},
              {{"id":"exe","file":"run.exe","shipped":true}},
              {{"id":"good","file":"Again.dll","shipped":true}}]}}"#
        ))
        .unwrap();
        let ids: Vec<_> = c.mods.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["good", "shipped"]);
        assert!(c.mods[0].downloadable() && !c.mods[1].downloadable());
    }

    #[test]
    fn the_draft_catalogue_is_valid() {
        let draft = include_str!("../../../docs/launcher/mods.json");
        let raw: Value = serde_json::from_str(draft).unwrap();
        let c = ModCatalog::parse(draft).unwrap();
        assert_eq!(c.mods.len(), raw["mods"].as_array().unwrap().len());
        assert!(ModCatalog::builtin().builtin);
    }
}
