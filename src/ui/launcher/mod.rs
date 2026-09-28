//! The launcher dashboard (root window): a left icon rail, a transparent top bar with
//! status pills, and full-bleed pages over darkened game art. See `ui/style.rs` for the
//! design system. The installer wizards open from here and keep their classic look.

mod play;
mod settings;
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
use crate::core::launcher::catalog::{Catalog, Platform};
use crate::core::launcher::game::{GameState, Monitor};
use crate::core::launcher::quest::QuestInfo;
use crate::core::launcher::store::{InstalledVersion, LauncherState};
use crate::core::launcher::versions::Step;

/// Wizards the dashboard can open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Open {
    PcInstall,
    PcUpdate,
    QuestInstall,
    QuestUpdate,
}

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
}

enum Msg {
    Catalog(Catalog),
    JobStep(String, Step),
    JobDone(String, JobResult),
    QuestInfo(Result<QuestInfo, UiError>),
    QuestAction(Result<(), UiError>),
    CacheDeleted(Vec<PathBuf>),
}

struct Job {
    label: String,
    fraction: Option<f32>,
    cancel: Arc<AtomicBool>,
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
    options_open: bool,
    clippy: Clippy,
    /// Snapshot mode: made-up state, never saved.
    pub demo: bool,
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
        self.dialogs.modern = true;
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
        let c = ctx.clone();
        self.monitor = Some(Monitor::start(move || c.request_repaint()));
        self.refresh_catalog(ctx);
    }

    fn refresh_catalog(&mut self, ctx: &egui::Context) {
        self.catalog_loading = true;
        self.worker
            .spawn(ctx, |tx| tx.send(Msg::Catalog(Catalog::load())));
    }

    /// After a wizard closes: pick up an install it made at the saved path.
    pub fn wizard_closed(&mut self) {
        if let Some(p) = crate::core::config::load_install_path() {
            let root = crate::core::paths::resolve_install_root(&p);
            if crate::core::paths::has_echo_install(&root)
                && self.state.add_external(&root, None).is_some()
            {
                self.save();
            }
        }
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
        self.quest_conn.poll(&mut self.dialogs);
        // Read the headset's version once it is connected.
        let ready = self.quest_conn.status == Some(Status::Ready);
        if ready && self.quest_info.is_none() && !self.quest_busy {
            self.quest_busy = true;
            self.worker.spawn(ctx, |tx| {
                let r = crate::core::launcher::quest::info()
                    .map_err(|e| UiError::from_anyhow(&e, "Quest"));
                tx.send(Msg::QuestInfo(r));
            });
        }
    }

    fn job_done(&mut self, id: &str, r: JobResult) {
        match r {
            JobResult::Installed(v) => {
                let name = v.name.clone();
                if self.state.selected_version().is_none() {
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
        label: &str,
        f: impl FnOnce(&AtomicBool, &mut dyn FnMut(Step)) -> JobResult + Send + 'static,
    ) {
        let cancel = Arc::new(AtomicBool::new(false));
        self.jobs.insert(
            id.to_string(),
            Job {
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

    /// Draws the dashboard; returns a wizard to open.
    pub fn show(&mut self, kit: &mut Kit) -> Option<Open> {
        let ctx = kit.ctx();
        if !self.started {
            self.start(&ctx);
        }
        self.poll(&ctx);

        // Backdrop: hero art on Play, a calm gradient elsewhere.
        match (self.page, self.play_platform) {
            (Page::Play, Platform::Pc) => kit.hero_backdrop("hero_pc.jpg", RAIL, 0.0, W - RAIL, H),
            (Page::Play, Platform::Quest) => {
                kit.hero_backdrop("hero_quest.jpg", RAIL, 0.0, W - RAIL, H)
            }
            _ => kit.plain_backdrop(RAIL, 0.0, W - RAIL, H),
        }

        let mut open = None;
        match self.page {
            Page::Play => open = play::show(self, kit, &ctx),
            Page::Versions => open = versions::show(self, kit, &ctx),
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
        if self.any_job() || self.quest_busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        open
    }

    fn rail(&mut self, kit: &mut Kit) {
        kit.fill(0.0, 0.0, RAIL, H, style::with_alpha(style::BG, 246));
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

    fn top_bar(&mut self, kit: &mut Kit, ctx: &egui::Context) {
        kit.text(
            X0,
            24.0,
            self.page.title(),
            style::display(16.0),
            style::TEXT,
        );
        // Right-aligned status pills.
        let game = self.game();
        let (gtext, gcolor) = match &game {
            GameState::NotRunning => ("Not running", style::TEXT_MUTED),
            GameState::InMatch { .. } => ("In a match", style::OK),
            _ => ("Running", style::OK),
        };
        let (qtext, qcolor) = match (self.quest_conn.checking, self.quest_conn.status) {
            (true, _) => ("Quest: checking...", style::TEXT_MUTED),
            (_, Some(Status::Ready)) => ("Quest connected", style::ACCENT),
            (_, Some(Status::Unauthorized)) => ("Quest: allow this PC", style::WARN),
            (_, Some(Status::Ambiguous)) => ("Several devices", style::WARN),
            (_, Some(Status::None)) => ("No Quest", style::TEXT_MUTED),
            (_, None) => ("Quest: check", style::TEXT_MUTED),
        };
        let gw = kit.pill_width(gtext, true);
        let qw = kit.pill_width(qtext, true);
        let qx = W - 32.0 - qw;
        let gx = qx - 8.0 - gw;
        kit.pill(gx, 21.0, gtext, gcolor, true);
        let gr = kit.rect(gx, 21.0, gw, 22.0);
        kit.hot("game-pill", gr, false, &game.label());
        kit.pill(qx, 21.0, qtext, qcolor, true);
        let r = kit.rect(qx, 21.0, qw, 22.0);
        if kit
            .hot(
                "quest-pill",
                r,
                !self.quest_conn.checking,
                "Check the Quest connection",
            )
            .0
            .clicked
        {
            self.check_quest(ctx, true);
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
