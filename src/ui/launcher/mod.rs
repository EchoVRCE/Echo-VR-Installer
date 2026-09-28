//! The launcher, in the installer's frame (`ui/frame.rs`): the blue status bar, a
//! sidebar listing the pages and the installer wizards, the magenta section box with the
//! page, and the bottom bar with the page's chips and buttons. Wizards open in its place.

mod play;
mod settings;
mod versions;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::dialogs::DialogHost;
use super::frame::{self, Mark, SideRow, CONTENT_W, CONTENT_X, SECTION_Y};
use super::kit::Kit;
use super::parts::{QuestConn, Worker};
use super::style::{self, Tips};
use super::theme;
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

/// Inner left edge and width of the section box, and its horizontal center.
const IX: f32 = CONTENT_X + 20.0;
const IW: f32 = CONTENT_W - 40.0;
const CX: f32 = CONTENT_X + CONTENT_W / 2.0;
/// Where a page's header banner sits (the wizards' content top + 8).
const HEADER_Y: f32 = 72.0;

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
    tips: Tips,
    pulse: frame::Pulse,
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

    /// Draws the launcher; returns a wizard to open.
    pub fn show(&mut self, kit: &mut Kit) -> Option<Open> {
        let ctx = kit.ctx();
        if !self.started {
            self.start(&ctx);
        }
        self.poll(&ctx);

        let quest = match self.page {
            Page::Play => self.play_platform == Platform::Quest,
            Page::Versions => self.versions_platform == Platform::Quest,
            _ => false,
        };
        frame::background(kit, if quest { "Echo2.jpg" } else { "EchoArena.jpg" });
        frame::boxes(kit);
        // Clippy climbs out from behind the bottom bar.
        if self.page == Page::Settings {
            kit.clipped(0.0, 0.0, frame::W, frame::BAR_Y, |k| {
                self.clippy
                    .draw(k, CONTENT_X + CONTENT_W - 220.0, frame::BAR_Y, 180.0)
            });
        }
        frame::bottom_bar(kit);

        let mut open = match self.page {
            Page::Play => play::show(self, kit, &ctx),
            Page::Versions => versions::show(self, kit, &ctx),
            Page::Settings => {
                settings::show(self, kit, &ctx);
                None
            }
            Page::Mods => {
                coming_soon(
                    kit,
                    "Mods & plugins",
                    "Turn DLL plugins and game tweaks on and off, per version.",
                );
                None
            }
            Page::Servers => {
                coming_soon(
                    kit,
                    "Servers",
                    "Browse, join and create EchoVRCE lobbies right from the launcher.",
                );
                None
            }
        };

        self.status_bar(kit, &ctx);
        if let Some(o) = self.sidebar(kit) {
            open = Some(o);
        }
        let tip = kit.tip.take();
        self.tips.show(&ctx, tip);
        if self.any_job() || self.quest_busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        open
    }

    /// The sidebar: the launcher's pages, then the installer wizards.
    fn sidebar(&mut self, kit: &mut Kit) -> Option<Open> {
        let pages = [
            (Page::Play, "Play Echo VR"),
            (Page::Versions, "Install and manage Echo VR versions"),
            (Page::Mods, "Plugins and tweaks"),
            (Page::Servers, "Lobbies and servers"),
            (Page::Settings, "Library folder, maintenance and about"),
        ];
        let rows: Vec<SideRow> = pages
            .iter()
            .map(|(p, tip)| SideRow {
                label: p.title(),
                mark: if self.page == *p {
                    Mark::Current
                } else {
                    Mark::Open
                },
                clickable: self.page != *p,
                tip,
            })
            .collect();
        let (picked, y) = frame::side_list(kit, "nav", SECTION_Y + 10.0, "Launcher", &rows);
        if let Some(i) = picked {
            self.page = pages[i].0;
        }

        let wizards = [
            (
                Open::PcInstall,
                "Install PC",
                "The step-by-step PC installer: download, licence patch, Revive setup",
            ),
            (
                Open::PcUpdate,
                "Update PC",
                "The step-by-step updater for any Echo VR folder",
            ),
            (
                Open::QuestInstall,
                "Install Quest",
                "Install Echo VR on your Quest over USB",
            ),
            (
                Open::QuestUpdate,
                "Update Quest",
                "Copy the latest game files to your Quest",
            ),
        ];
        let rows: Vec<SideRow> = wizards
            .iter()
            .map(|(_, label, tip)| SideRow {
                label,
                mark: Mark::Open,
                clickable: true,
                tip,
            })
            .collect();
        let (picked, _) = frame::side_list(kit, "wizards", y + 8.0, "Installer", &rows);
        picked.map(|i| wizards[i].0)
    }

    /// The installer's blue status bar: pulsing while busy, green while the game runs,
    /// with the Quest connection as a chip at its right end.
    fn status_bar(&mut self, kit: &mut Kit, ctx: &egui::Context) {
        let game = self.game();
        let busy = self.any_job() || self.quest_busy || self.quest_conn.checking;
        let p = self.pulse.tick(busy);
        let fill = if game.is_running() {
            theme::STATUS_DONE
        } else if busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            frame::pulse_fill(p)
        } else {
            theme::STATUS_IDLE
        };
        let mut status = game.label();
        if let Some(v) = self.state.selected_version() {
            status = format!("{status}   •   {}", v.name);
        }
        frame::status_bar(kit, &status, fill);

        let (qtext, on) = match (self.quest_conn.checking, self.quest_conn.status) {
            (true, _) => ("Quest: checking", false),
            (_, Some(Status::Ready)) => ("Quest connected", true),
            (_, Some(Status::Unauthorized)) => ("Quest: allow this PC", false),
            (_, Some(Status::Ambiguous)) => ("Several devices", false),
            (_, Some(Status::None)) => ("No Quest", false),
            (_, None) => ("Quest: check", false),
        };
        let qw = frame::badge_width(kit, qtext);
        let qx = CONTENT_X + CONTENT_W - qw - 6.0;
        frame::badge(kit, qx, 16.0, qtext, on);
        if !self.quest_conn.checking
            && kit
                .hand_area(
                    "quest-chip",
                    qx,
                    16.0,
                    qw,
                    20.0,
                    "Check the Quest connection",
                )
                .clicked
        {
            self.check_quest(ctx, true);
        }
    }
}

/// A page's header banner, centered in the section box like the wizards'.
fn header(kit: &Kit, text: &str, y: f32) {
    kit.header(text, CX - 225.0, y, 450.0, 55.0);
}

/// Centered, wrapped Arial text in the section box.
fn para(kit: &Kit, text: &str, y: f32, size: f32, color: egui::Color32) {
    kit.text_center(
        IX,
        y,
        IW,
        size * 1.3,
        text,
        theme::arial(size),
        color,
        Some(IW - 80.0),
    );
}

/// Pages that are not built yet.
fn coming_soon(kit: &mut Kit, title: &str, text: &str) {
    header(kit, title, HEADER_Y);
    para(kit, text, 150.0, 14.0, style::TEXT);
    let w = frame::badge_width(kit, "Coming soon");
    frame::badge(kit, CX - w / 2.0, 182.0, "Coming soon", false);
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
