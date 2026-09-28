//! The launcher (root window): a left icon rail, the installer's blue status bar, and
//! pages over the game art. See `ui/style.rs` for the widgets. Installing, patching,
//! SteamVR setup and the Quest all run inline (`setup.rs`).

mod play;
mod settings;
mod setup;
mod versions;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::dialogs::DialogHost;
use super::kit::Kit;
use super::parts::{QuestConn, Worker};
use super::style::{self, Icon};
use super::tipbox::Clippy;
use crate::core::adb::devices::Status;
use crate::core::error::UiError;
use crate::core::launcher::catalog::{Catalog, Platform, VersionEntry};
use crate::core::launcher::game::{GameState, Monitor};
use crate::core::launcher::quest::QuestInfo;
use crate::core::launcher::store::{InstalledVersion, LauncherState, Target};
use crate::core::launcher::versions::Step;

pub const CREDITS: &str = "Copyright for Echo VR is by Meta/Ready at Dawn!\n\
This launcher is not at all associated with them!\n\n\
Special thanks to Sick and SirDominik for some of the backgrounds!\n\
Special thanks to F-A-N-G-O-R-N for getting me into Java and helping with this project.\n\
I know you still feel shame when you have to look at my source code.\n\
Special thanks to Leon(leon1273) for contributing and cleaning stuff in my code\n\
This tool is still in early alpha!\n\
If you have problems, contact me on Discord 'marshmallow_mia'.";

pub const W: f32 = 1280.0;
pub const H: f32 = 720.0;
/// Width of the left navigation rail.
const RAIL: f32 = 72.0;
/// Left edge and width of page content.
const X0: f32 = RAIL + 32.0;
const CW: f32 = W - X0 - 32.0;
/// The page-title banner under the status bar, and where controls next to it start.
const TITLE_Y: f32 = 56.0;
const TITLE_W: f32 = 260.0;
const BESIDE_TITLE: f32 = X0 + TITLE_W + 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Play,
    Versions,
    Mods,
    Servers,
    Settings,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Play => "Play",
            Page::Versions => "Versions",
            Page::Mods => "Mods",
            Page::Servers => "Servers",
            Page::Settings => "Settings",
        }
    }
}

enum JobResult {
    Installed(InstalledVersion),
    Updated,
    Verified(Vec<String>),
    /// `None` = cancelled.
    Failed(Option<UiError>),
    /// The licence patch was applied.
    Patched,
    /// Discord authorization for the patch failed.
    OAuthFailed(crate::core::oauth::OAuthError),
    /// Revive (SteamVR) is installed.
    ReviveReady,
    QuestInstalled,
    QuestUpdated,
    /// The headset's APK doesn't match the update: offer a reinstall (the text says why).
    QuestNeedsReinstall(String),
}

enum Msg {
    Catalog(Catalog),
    JobStep(String, Step),
    JobDone(String, JobResult),
    QuestInfo(Result<QuestInfo, UiError>),
    QuestAction(Result<(), UiError>),
    CacheDeleted(Vec<PathBuf>),
    /// A job needs administrator rights: ask, then answer on the channel.
    Consent(std::sync::mpsc::SyncSender<bool>),
}

struct Job {
    /// What runs, for the status bar ("Installing Echo VR (PC, latest)").
    title: String,
    /// The latest progress line.
    label: String,
    fraction: Option<f32>,
    cancel: Arc<AtomicBool>,
}

/// Snapshot mode: extra states to capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapVariant {
    /// The PLAY menu is open.
    PlayMenu,
    /// A version that is not installed is selected.
    NotInstalled,
    /// That version is being installed.
    Installing,
    /// The first-run setup card.
    Setup,
    /// Its second step (how you play).
    SetupHeadset,
    /// The launch options are shown under PLAY.
    LaunchOptions,
    /// The Quest side of Play, with Echo VR installed on the headset.
    QuestSide,
    /// A new player's version that still needs the licence patch.
    NeedsPatch,
}

#[derive(Default)]
pub struct Dashboard {
    pub page: Page,
    state: LauncherState,
    catalog: Option<Catalog>,
    catalog_loading: bool,
    jobs: HashMap<String, Job>,
    pub dialogs: DialogHost,
    worker: Worker<Msg>,
    monitor: Option<Monitor>,
    child: Option<std::process::Child>,
    /// Play page: PC or Quest.
    play_platform: Platform,
    /// Versions page: PC or Quest.
    versions_platform: Platform,
    quest_conn: QuestConn,
    quest_info: Option<QuestInfo>,
    quest_busy: bool,
    versions_scroll: f32,
    library_field: String,
    pending_remove: Option<String>,
    pending_repair: Option<String>,
    /// Last update result per version, shown on the Play page's Updates card.
    update_note: HashMap<String, String>,
    clippy: Clippy,
    /// Snapshot mode: made-up state, never saved.
    pub demo: bool,
    pub snap_variant: Option<SnapVariant>,
    applied_variant: Option<SnapVariant>,
    /// A lobby link found on the clipboard when the window gained focus.
    clip_lobby: Option<String>,
    /// When the game was first seen running.
    game_since: Option<std::time::Instant>,
    /// The Quest was checked quietly once already.
    quest_auto_checked: bool,
    /// A full-window card on top (first-run setup, patch from a link).
    overlay: Option<setup::Overlay>,
    /// A job waiting for the administrator-rights answer.
    consent: Option<std::sync::mpsc::SyncSender<bool>>,
    /// Where Revive is installed (checked at most every 5 s).
    revive_cache: Option<(Option<String>, std::time::Instant)>,
    /// Free space in the library: (library, bytes, when measured).
    free_cache: Option<(String, Option<u64>, std::time::Instant)>,
    started: bool,
    deleting_cache: bool,
}

impl Dashboard {
    fn save(&self) {
        if self.demo {
            return;
        }
        if let Err(e) = self.state.save() {
            tracing::error!("saving launcher state failed: {e:#}");
        }
    }

    /// First frame: load state, import existing installs, start the monitor and catalogue.
    fn start(&mut self, ctx: &egui::Context) {
        self.started = true;
        self.state = if self.demo {
            demo_state()
        } else {
            LauncherState::load()
        };
        if !self.state.imported {
            let n = self.state.import_existing();
            tracing::info!("imported {n} existing install(s)");
            self.save();
        }
        self.library_field = self.state.library.clone();
        if !self.state.setup_done && !self.demo {
            self.overlay = Some(setup::Overlay::Setup { step: 0 });
        }
        if self.demo {
            self.catalog = Some(demo_catalog());
            return;
        }
        let c = ctx.clone();
        self.monitor = Some(Monitor::start(move || c.request_repaint()));
        self.refresh_catalog(ctx);
        self.read_clipboard_lobby();
    }

    /// Offers a lobby link from the clipboard (checked when the window gains focus).
    fn read_clipboard_lobby(&mut self) {
        let clip = arboard::Clipboard::new()
            .ok()
            .and_then(|mut c| c.get_text().ok())
            .map(|t| t.trim().to_string());
        self.clip_lobby = clip.filter(|t| {
            t.len() < 300
                && crate::core::launcher::launch::lobby_uuid(t).is_some()
                && crate::core::launcher::launch::lobby_uuid(t)
                    != crate::core::launcher::launch::lobby_uuid(&self.state.last_lobby)
        });
    }

    /// Free bytes where new versions are installed (measured at most every 10 s).
    fn free_bytes(&mut self) -> Option<u64> {
        if self.demo {
            return Some(120_000_000_000);
        }
        let lib = self.state.library.clone();
        match &self.free_cache {
            Some((l, b, at)) if *l == lib && at.elapsed().as_secs() < 10 => *b,
            _ => {
                let b = crate::core::platform::free_space(std::path::Path::new(&lib));
                self.free_cache = Some((lib, b, std::time::Instant::now()));
                b
            }
        }
    }

    /// Where Revive is installed, if it is.
    fn revive_dir(&mut self) -> Option<String> {
        if self.demo {
            return None;
        }
        match &self.revive_cache {
            Some((dir, at)) if at.elapsed().as_secs() < 5 => dir.clone(),
            _ => {
                let dir = crate::core::revive::find_revive_dir();
                self.revive_cache = Some((dir.clone(), std::time::Instant::now()));
                dir
            }
        }
    }

    /// What PLAY acts on.
    fn target(&self) -> Target {
        let demo = self.demo;
        self.state.target(self.catalog.as_ref(), |root| {
            demo || crate::core::paths::has_echo_install(root)
        })
    }

    /// Snapshot mode: puts the dashboard into `snap_variant`'s state.
    fn apply_snap_variant(&mut self, ctx: &egui::Context) {
        if self.snap_variant == self.applied_variant {
            return;
        }
        self.applied_variant = self.snap_variant;
        self.jobs.clear();
        self.overlay = None;
        self.state.owner = Some(true);
        self.state.selected = Some("pc-latest".into());
        self.state.show_launch_options = false;
        self.play_platform = Platform::Pc;
        self.quest_conn.status = None;
        self.quest_info = None;
        let open = egui::Id::new(("style", play::VERSION_KEY)).with("open");
        ctx.data_mut(|d| d.insert_temp(open, false));
        match self.snap_variant {
            Some(SnapVariant::PlayMenu) => {
                ctx.data_mut(|d| d.insert_temp(open, true));
            }
            Some(SnapVariant::NotInstalled) => self.state.selected = Some("pc-34.4".into()),
            Some(SnapVariant::Installing) => {
                self.state.selected = Some("pc-34.4".into());
                self.jobs.insert(
                    "pc-34.4".into(),
                    Job {
                        title: "Installing Echo VR 34.4 (PC)".into(),
                        label: "Downloading... 42.0%".into(),
                        fraction: Some(0.42),
                        cancel: Arc::new(AtomicBool::new(false)),
                    },
                );
            }
            Some(SnapVariant::Setup) => {
                self.state.owner = None;
                self.overlay = Some(setup::Overlay::Setup { step: 0 });
            }
            Some(SnapVariant::SetupHeadset) => {
                self.overlay = Some(setup::Overlay::Setup { step: 1 });
            }
            Some(SnapVariant::NeedsPatch) => self.state.owner = Some(false),
            Some(SnapVariant::LaunchOptions) => self.state.show_launch_options = true,
            Some(SnapVariant::QuestSide) => {
                self.play_platform = Platform::Quest;
                self.quest_conn.status = Some(Status::Ready);
                self.quest_info = Some(QuestInfo {
                    device: Some("Meta Quest 3 (2G0YC5ZF8R0123)".into()),
                    installed: true,
                    marker: Some(crate::core::quest_update::Marker {
                        base_apk: Some("r15_26-06-23.apk".into()),
                        ..Default::default()
                    }),
                });
            }
            None => {}
        }
    }

    fn refresh_catalog(&mut self, ctx: &egui::Context) {
        self.catalog_loading = true;
        self.worker
            .spawn(ctx, |tx| tx.send(Msg::Catalog(Catalog::load())));
    }

    fn game(&self) -> GameState {
        self.monitor.as_ref().map(Monitor::get).unwrap_or_default()
    }

    fn check_quest(&mut self, ctx: &egui::Context, interactive: bool) {
        self.quest_info = None;
        self.quest_conn.check(ctx, interactive);
    }

    fn poll(&mut self, ctx: &egui::Context) {
        // Forget our child once it exited.
        if let Some(c) = self.child.as_mut() {
            if !matches!(c.try_wait(), Ok(None)) {
                self.child = None;
            }
        }
        for m in self.worker.drain() {
            match m {
                Msg::Catalog(c) => {
                    self.catalog = Some(c);
                    self.catalog_loading = false;
                }
                Msg::JobStep(id, step) => {
                    if let Some(j) = self.jobs.get_mut(&id) {
                        match step {
                            Step::Status(s) => {
                                // The downloader reports progress as "12.34%" status lines too.
                                j.fraction = s
                                    .strip_suffix('%')
                                    .and_then(|p| p.parse::<f32>().ok())
                                    .map(|p| p / 100.0);
                                j.label = s;
                            }
                            Step::Percent(p) => {
                                j.fraction = Some(p as f32 / 100.0);
                                j.label = format!("Downloading... {p:.1}%");
                            }
                        }
                    }
                }
                Msg::JobDone(id, r) => {
                    self.jobs.remove(&id);
                    self.job_done(&id, r);
                }
                Msg::QuestInfo(r) => {
                    self.quest_busy = false;
                    match r {
                        Ok(i) => self.quest_info = Some(i),
                        Err(e) => {
                            self.quest_info = None;
                            self.dialogs.error_ui(&e);
                        }
                    }
                }
                Msg::QuestAction(r) => {
                    self.quest_busy = false;
                    if let Err(e) = r {
                        self.dialogs.error_ui(&e);
                    }
                }
                Msg::Consent(tx) => {
                    self.consent = Some(tx);
                    self.dialogs.confirm(
                        setup::CONSENT_KEY,
                        "Administrator rights required",
                        "This step needs administrator rights (it installs into Program Files).\n\nStart the privileged helper now? Windows will ask you to confirm.",
                        crate::ui::dialogs::Icon::Question,
                    );
                }
                Msg::CacheDeleted(failed) => {
                    self.deleting_cache = false;
                    let mut msg = String::from("The cached files have been deleted.");
                    if !failed.is_empty() {
                        msg.push_str("\n\nThese could not be deleted (in use or no permission):");
                        for f in failed.iter().take(6) {
                            msg.push_str(&format!("\n{}", f.display()));
                        }
                    }
                    self.dialogs.info("Deleting done", &msg);
                }
            }
        }
        if let Some(a) = self.dialogs.take(setup::CONSENT_KEY) {
            if let Some(tx) = self.consent.take() {
                let _ = tx.send(a.is_yes());
            }
        }
        if self
            .dialogs
            .take(setup::JOIN_KEY)
            .is_some_and(|a| a.is_yes())
        {
            crate::core::platform::open_url(crate::core::oauth::INVITE_URL);
        }
        self.quest_conn.poll(&mut self.dialogs);
        // Read the headset's version once it is connected.
        if self
            .dialogs
            .take(setup::QUEST_INSTALL_KEY)
            .is_some_and(|a| a.is_yes())
            || self
                .dialogs
                .take(setup::QUEST_REINSTALL_KEY)
                .is_some_and(|a| a.is_yes())
        {
            let source = setup::quest_source(self);
            setup::quest_install(self, ctx, source);
        }
        let ready = self.quest_conn.status == Some(Status::Ready);
        // Probing while a Quest job runs would trip over its adb restarts.
        let quest_job = self.jobs.contains_key(setup::QUEST_JOB);
        if ready && self.quest_info.is_none() && !self.quest_busy && !quest_job {
            self.quest_busy = true;
            self.worker.spawn(ctx, |tx| {
                let r = crate::core::launcher::quest::info()
                    .map_err(|e| UiError::from_anyhow(&e, "Quest"));
                tx.send(Msg::QuestInfo(r));
            });
        }
    }

    fn job_done(&mut self, id: &str, r: JobResult) {
        if id == setup::QUEST_JOB {
            // Read the headset again once the job is over.
            self.quest_info = None;
        }
        match r {
            JobResult::Installed(v) => {
                let name = v.name.clone();
                if self.state.selected.is_none() {
                    self.state.selected = Some(v.id.clone());
                }
                self.state.upsert(v);
                self.save();
                self.dialogs.info(
                    "Installed",
                    &format!("{name} is installed and ready to play."),
                );
            }
            JobResult::Updated => {
                self.update_note.insert(id.to_string(), "Up to date".into());
            }
            JobResult::Verified(bad) if bad.is_empty() => {
                self.update_note
                    .insert(id.to_string(), "All files intact".into());
                self.dialogs.info("Verify", "All game files are intact.");
            }
            JobResult::Verified(bad) => {
                let mut msg = format!("{} file(s) are missing or modified:\n", bad.len());
                for b in bad.iter().take(8) {
                    msg.push_str(&format!("\n  {b}"));
                }
                if bad.len() > 8 {
                    msg.push_str("\n  ...");
                }
                msg.push_str("\n\nRepair them now?");
                versions::ask_repair(self, id, &msg);
            }
            JobResult::Patched => {
                if let Some(v) = self.state.versions.iter_mut().find(|v| v.id == id) {
                    v.patched = true;
                }
                self.save();
                self.dialogs.info(
                    "Licence patch applied",
                    "Your personal licence patch is in place. Have fun!",
                );
            }
            JobResult::OAuthFailed(e) => {
                use crate::core::oauth::OAuthError;
                match (&e, e.dialog()) {
                    (OAuthError::NotInGuild(_), Some((title, msg))) => self.dialogs.options(
                        setup::JOIN_KEY,
                        title,
                        &msg,
                        crate::ui::dialogs::Icon::Info,
                        &["Join Server", "Close"],
                    ),
                    (_, Some((title, msg))) => self.dialogs.error(title, &msg, Default::default()),
                    (_, None) => {}
                }
            }
            JobResult::ReviveReady => {
                self.revive_cache = None;
                self.dialogs.info(
                    "SteamVR is ready",
                    "Revive is installed. PLAY now starts Echo VR through SteamVR.",
                );
            }
            JobResult::QuestInstalled => self.dialogs.info(
                "Installed",
                "Echo VR is installed on your Quest and up to date.",
            ),
            JobResult::QuestUpdated => self
                .dialogs
                .info("Up to date", "Your Quest has the latest update."),
            JobResult::QuestNeedsReinstall(detail) => self.dialogs.options(
                setup::QUEST_REINSTALL_KEY,
                "Echo VR version mismatch",
                &format!("{detail}\n\nReinstall Echo VR on your Quest to continue."),
                crate::ui::dialogs::Icon::Warning,
                &["Reinstall Echo VR", "Cancel"],
            ),
            JobResult::Failed(None) => {}
            JobResult::Failed(Some(e)) => {
                self.update_note
                    .insert(id.to_string(), "Last update failed".into());
                self.dialogs.error_ui(&e);
            }
        }
    }

    fn start_job(
        &mut self,
        ctx: &egui::Context,
        id: &str,
        title: &str,
        label: &str,
        f: impl FnOnce(&AtomicBool, &mut dyn FnMut(Step)) -> JobResult + Send + 'static,
    ) {
        let cancel = Arc::new(AtomicBool::new(false));
        self.jobs.insert(
            id.to_string(),
            Job {
                title: title.to_string(),
                label: label.to_string(),
                fraction: None,
                cancel: cancel.clone(),
            },
        );
        let id = id.to_string();
        self.worker.spawn(ctx, move |tx| {
            let mut on = |s: Step| tx.send(Msg::JobStep(id.clone(), s));
            let r = f(&cancel, &mut on);
            tx.send(Msg::JobDone(id, r));
        });
    }

    fn cancel_job(&mut self, id: &str) {
        if let Some(j) = self.jobs.get(id) {
            j.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn any_job(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// Draws the launcher.
    pub fn show(&mut self, kit: &mut Kit) {
        let ctx = kit.ctx();
        if !self.started {
            self.start(&ctx);
        }
        self.poll(&ctx);
        if self.demo {
            self.apply_snap_variant(&ctx);
        } else if ctx.input(|i| {
            i.raw
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::WindowFocused(true)))
        }) {
            self.read_clipboard_lobby();
        }
        // Track how long the game has been running.
        match (self.game().is_running(), self.game_since) {
            (true, None) => self.game_since = Some(std::time::Instant::now()),
            (false, Some(_)) => self.game_since = None,
            _ => {}
        }

        // Backdrop: hero art on Play, a calm gradient elsewhere.
        match (self.page, self.play_platform) {
            (Page::Play, Platform::Pc) => kit.hero_backdrop("hero_pc.jpg", RAIL, 0.0, W - RAIL, H),
            (Page::Play, Platform::Quest) => {
                kit.hero_backdrop("hero_quest.jpg", RAIL, 0.0, W - RAIL, H)
            }
            _ => kit.plain_backdrop(RAIL, 0.0, W - RAIL, H),
        }

        // The dashboard stays visible but inert under an overlay card.
        let blocked = kit.blocked;
        if self.overlay.is_some() {
            kit.blocked = true;
        }
        match self.page {
            Page::Play => play::show(self, kit, &ctx),
            Page::Versions => versions::show(self, kit, &ctx),
            Page::Settings => settings::show(self, kit, &ctx),
            Page::Mods => empty_state(
                kit,
                Icon::Mods,
                "Mods & plugins",
                "Enable and disable DLL plugins and game tweaks per version.",
            ),
            Page::Servers => empty_state(
                kit,
                Icon::Globe,
                "Servers",
                "Browse, join and create EchoVRCE lobbies right from the launcher.",
            ),
        }

        self.top_bar(kit, &ctx);
        self.rail(kit);
        kit.blocked = blocked;
        setup::draw_overlay(self, kit, &ctx);
        if self.any_job() || self.quest_busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }

    fn rail(&mut self, kit: &mut Kit) {
        // The installer's wine sidebar.
        kit.fill(0.0, 0.0, RAIL, H, style::with_alpha(style::BG, 200));
        kit.fill(0.0, 0.0, RAIL, H, crate::ui::theme::SIDEBAR_FILL);
        kit.fill(RAIL - 1.0, 0.0, 1.0, H, style::BORDER);
        kit.image("icon.png", 18.0, 18.0, 36.0, 36.0);
        let items = [
            (Page::Play, Icon::Play, "Play", "Play Echo VR"),
            (
                Page::Versions,
                Icon::Download,
                "Versions",
                "Install and manage Echo VR versions",
            ),
            (Page::Mods, Icon::Mods, "Mods", "Plugins and tweaks"),
            (Page::Servers, Icon::Globe, "Servers", "Lobbies and servers"),
        ];
        for (i, (page, icon, label, tip)) in items.iter().enumerate() {
            if kit.nav_item(
                &format!("nav-{i}"),
                *icon,
                label,
                0.0,
                84.0 + i as f32 * 68.0,
                RAIL,
                60.0,
                self.page == *page,
                tip,
            ) {
                self.page = *page;
            }
        }
        if kit.nav_item(
            "nav-settings",
            Icon::Gear,
            "Settings",
            0.0,
            H - 76.0,
            RAIL,
            60.0,
            self.page == Page::Settings,
            "Settings and about",
        ) {
            self.page = Page::Settings;
        }
    }

    /// The installer's blue status bar (pulsing while busy, green while the game runs)
    /// with the Quest chip at its right end, and the page's banner title below it.
    fn top_bar(&mut self, kit: &mut Kit, ctx: &egui::Context) {
        let game = self.game();
        let busy = self.any_job() || self.quest_busy || self.quest_conn.checking;
        let fill = if game.is_running() {
            crate::ui::theme::STATUS_DONE
        } else if busy {
            let p = style::pulse(ctx);
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            egui::Color32::from_rgb(
                (50.0 + p * 40.0) as u8,
                (90.0 + p * 50.0) as u8,
                (150.0 + p * 60.0) as u8,
            )
        } else {
            crate::ui::theme::STATUS_IDLE
        };
        let (bx, bw) = (RAIL + 16.0, W - RAIL - 32.0);
        kit.round_box(bx, 10.0, bw, 32.0, 8.0, fill, Some(style::BORDER));
        let status = if let Some(j) = self.jobs.values().next() {
            match j.fraction {
                Some(f) => format!("{}   •   {:.0}%", j.title, f * 100.0),
                None => format!("{}   •   {}", j.title, j.label),
            }
        } else if let (true, Some(since)) = (game.is_running(), self.game_since) {
            let mins = since.elapsed().as_secs() / 60;
            ctx.request_repaint_after(std::time::Duration::from_secs(20));
            if mins == 0 {
                game.label()
            } else {
                format!("{}   •   {mins} min", game.label())
            }
        } else {
            game.label()
        };
        kit.text_center(
            bx,
            10.0,
            bw,
            32.0,
            &status,
            style::bold(14.0),
            style::TEXT,
            None,
        );

        let (qtext, qcolor) = match (self.quest_conn.checking, self.quest_conn.status) {
            (true, _) => ("Quest: checking...", style::CHIP_OFF),
            (_, Some(Status::Ready)) => ("Quest connected", style::OK),
            (_, Some(Status::Unauthorized)) => ("Quest: allow this PC", style::WARN),
            (_, Some(Status::Ambiguous)) => ("Quest: pick a device", style::WARN),
            (_, Some(Status::None)) => ("Quest: not connected", style::CHIP_OFF),
            (_, None) => ("Quest: not checked", style::CHIP_OFF),
        };
        let qw = kit.pill_width(qtext, true);
        let qx = bx + bw - qw - 5.0;
        kit.pill(qx, 15.0, qtext, qcolor, true);
        let r = kit.rect(qx, 15.0, qw, 22.0);
        if kit
            .hot(
                "quest-pill",
                r,
                !self.quest_conn.checking,
                "Check the USB connection to your Quest",
            )
            .0
            .clicked
        {
            self.check_quest(ctx, true);
        }

        if self.page != Page::Play {
            kit.banner(X0, TITLE_Y, TITLE_W, 40.0, self.page.title(), 16.0);
        }
    }
}

/// A centered card for pages that are not built yet.
fn empty_state(kit: &mut Kit, icon: Icon, title: &str, text: &str) {
    let (w, h) = (520.0, 224.0);
    let x = X0 + (CW - w) / 2.0;
    let y = 200.0;
    kit.card(x, y, w, h);
    let top = kit.rect(x + w / 2.0 - 20.0, y + 32.0, 40.0, 40.0).min;
    style::icon_at(kit.ui.painter(), icon, top, 40.0, style::ACCENT);
    kit.text_center(
        x,
        y + 88.0,
        w,
        28.0,
        title,
        style::display(18.0),
        style::TEXT,
        None,
    );
    kit.text_center(
        x + 40.0,
        y + 122.0,
        w - 80.0,
        40.0,
        text,
        style::body(14.0),
        style::TEXT_DIM,
        Some(w - 80.0),
    );
    let pw = kit.pill_width("Coming soon", false);
    kit.pill(
        x + (w - pw) / 2.0,
        y + 176.0,
        "Coming soon",
        style::ACCENT_2,
        false,
    );
}

/// Two made-up versions for UI snapshots.
fn demo_state() -> LauncherState {
    let mut s = LauncherState {
        imported: true,
        ..Default::default()
    };
    s.versions.push(InstalledVersion {
        id: "pc-latest".into(),
        name: "Echo VR (PC, latest)".into(),
        root: "C:/EchoVR/versions/pc-latest".into(),
        catalog_id: Some("pc-latest".into()),
        ..Default::default()
    });
    s.versions.push(InstalledVersion {
        id: "existing".into(),
        name: "Existing install".into(),
        root: "C:/Program Files/Oculus/Software/Software".into(),
        external: true,
        ..Default::default()
    });
    s.selected = Some("pc-latest".into());
    s.profile.windowed = true;
    s
}

/// The built-in catalogue plus an older build that is not installed, for snapshots.
fn demo_catalog() -> Catalog {
    let mut c = Catalog::builtin();
    c.versions.push(VersionEntry {
        id: "pc-34.4".into(),
        name: "Echo VR 34.4 (PC)".into(),
        channel: "archive".into(),
        platform: Platform::Pc,
        url: "ready-at-dawn-echo-arena.zip".into(),
        size: Some(4_270_000_000),
        notes: "The last official build.".into(),
        ..Default::default()
    });
    c
}
