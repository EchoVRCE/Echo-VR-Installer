//! The launcher dashboard: the root window. Left navigation, status bar, a page area,
//! the TipBox and the credits badge. The installer wizards open from it.

mod play;
mod settings;
mod versions;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use egui::{vec2, Color32, Stroke};

use super::dialogs::DialogHost;
use super::kit::{Btn, Kit};
use super::parts::{QuestConn, Worker};
use super::theme;
use super::tipbox::{self, TipBox};
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
/// The page area inside the content box.
const PAGE_X: f32 = 250.0;
const PAGE_Y: f32 = 62.0;
const PAGE_W: f32 = 1010.0;

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
    const ALL: [Page; 5] = [
        Page::Play,
        Page::Versions,
        Page::Mods,
        Page::Servers,
        Page::Settings,
    ];

    fn label(self) -> &'static str {
        match self {
            Page::Play => "Play",
            Page::Versions => "Versions",
            Page::Mods => "Mods",
            Page::Servers => "Servers",
            Page::Settings => "Settings",
        }
    }

    fn tip(self) -> &'static str {
        match self {
            Page::Play => "Pick a version and a launch mode, then play",
            Page::Versions => "Install, update, verify and remove Echo VR versions",
            Page::Mods => "Enable and disable plugins and tweaks (coming soon)",
            Page::Servers => "Browse, join and create lobbies (coming soon)",
            Page::Settings => "Library folder, cache and logs",
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
    tipbox: TipBox,
    worker: Worker<Msg>,
    monitor: Option<Monitor>,
    child: Option<std::process::Child>,
    /// Play page: PC or Quest.
    play_platform: Platform,
    /// Versions page: PC or Quest.
    versions_platform: Platform,
    quest_conn: QuestConn,
    quest_conn_started: bool,
    quest_info: Option<QuestInfo>,
    quest_busy: bool,
    list_scroll: f32,
    versions_scroll: f32,
    library_field: String,
    pending_remove: Option<String>,
    pending_repair: Option<String>,
    started: bool,
    deleting_cache: bool,
}

impl Dashboard {
    fn save(&self) {
        if let Err(e) = self.state.save() {
            tracing::error!("saving launcher state failed: {e:#}");
        }
    }

    /// First frame: load state, import existing installs, start the monitor and catalogue.
    fn start(&mut self, ctx: &egui::Context) {
        self.started = true;
        self.state = LauncherState::load();
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

    fn poll(&mut self) {
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
                        j.label = match step {
                            Step::Status(s) => s,
                            Step::Percent(p) => format!("{p:.2}%"),
                        };
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
    }

    fn job_done(&mut self, id: &str, r: JobResult) {
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
                let name = self
                    .state
                    .version(id)
                    .map(|v| v.name.clone())
                    .unwrap_or_default();
                self.dialogs
                    .info("Up to date", &format!("{name} is up to date."));
            }
            JobResult::Verified(bad) if bad.is_empty() => {
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
            JobResult::Failed(Some(e)) => self.dialogs.error_ui(&e),
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
        self.poll();

        let tip_x = PAGE_X - 10.0 + ((PAGE_W + 20.0 - tipbox::W) / 2.0).floor();
        let tip_y = 527.0;
        kit.image("Echox720.png", 0.0, 0.0, W, H);
        self.tipbox.draw_clippy(kit, tip_x, tip_y);

        // Status bar.
        let game = self.game();
        let fill = if game.is_running() {
            theme::STATUS_DONE
        } else {
            theme::STATUS_IDLE
        };
        kit.round_box(
            240.0,
            10.0,
            1030.0,
            32.0,
            8.0,
            fill,
            Some(theme::BOX_BORDER),
        );
        let mut status = game.label();
        if let Some(v) = self.state.selected_version() {
            status = format!("{status}   •   {}", v.name);
        }
        kit.text_center(
            240.0,
            10.0,
            1030.0,
            32.0,
            &status,
            theme::arial_bold(14.0),
            theme::WHITE,
            None,
        );

        // Navigation.
        kit.section_box(10.0, 52.0, 220.0, 658.0, 15.0, theme::SIDEBAR_FILL);
        kit.text_center(
            10.0,
            60.0,
            220.0,
            30.0,
            "Echo VR",
            theme::conthrax(22.0),
            theme::WHITE,
            None,
        );
        kit.text_center(
            10.0,
            88.0,
            220.0,
            20.0,
            "Launcher",
            theme::conthrax(13.0),
            theme::LIGHT_GRAY,
            None,
        );
        for (i, p) in Page::ALL.iter().enumerate() {
            let y = 124.0 + i as f32 * 54.0;
            if *p == self.page {
                kit.round_box(
                    11.0,
                    y - 4.0,
                    218.0,
                    46.0,
                    10.0,
                    theme::rgba(0, 180, 0, 150),
                    None,
                );
            }
            if kit.button(
                &format!("nav-{i}"),
                Btn::Middle,
                p.label(),
                16.0,
                14.0,
                y,
                true,
                p.tip(),
            ) {
                self.page = *p;
            }
        }
        kit.text_center(
            10.0,
            684.0,
            220.0,
            20.0,
            crate::version::VERSION_TITLE,
            theme::arial(11.0),
            theme::LIGHT_GRAY,
            None,
        );

        // Page.
        kit.section_box(240.0, 52.0, 1030.0, 460.0, 15.0, theme::SECTION_FILL);
        let mut open = None;
        kit.at(PAGE_X, PAGE_Y, |k| {
            open = match self.page {
                Page::Play => play::show(self, k, &ctx),
                Page::Versions => versions::show(self, k, &ctx),
                Page::Settings => settings::show(self, k, &ctx),
                Page::Mods => {
                    coming_soon(k, "Mods", "Enable and disable DLL plugins and game tweaks per version.\nComing in the next launcher update.");
                    None
                }
                Page::Servers => {
                    coming_soon(k, "Servers", "Browse, join and create EchoVRCE lobbies.\nComing in a later launcher update.");
                    None
                }
            };
        });

        self.credits(kit);
        self.tipbox.draw(kit, tip_x, tip_y);
        if self.any_job() || self.quest_busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        open
    }

    fn credits(&mut self, kit: &mut Kit) {
        let (bx, by, s) = (W - 40.0 - 20.0, 660.0, 40.0);
        let c = kit.rect(bx, by, s, s).center();
        let p = kit.ui.painter();
        p.circle_filled(c, s / 2.0 - 1.0, Color32::from_rgb(200, 0, 150));
        p.circle_stroke(
            c,
            s / 2.0 - 1.5,
            Stroke::new(1.8, theme::rgba(255, 255, 255, 190)),
        );
        p.circle_filled(
            c + vec2(0.0, (s * 0.27).round() - s / 2.0),
            3.0,
            Color32::WHITE,
        );
        p.line_segment(
            [
                c + vec2(0.0, (s * 0.43).round() - s / 2.0),
                c + vec2(0.0, (s * 0.73).round() - s / 2.0),
            ],
            Stroke::new(4.4, Color32::WHITE),
        );
        if kit
            .hand_area("credits", bx, by, s, s, "About this launcher & credits")
            .clicked
        {
            self.dialogs.info("Credits", CREDITS);
        }
        if kit.area("easter-egg", 10.0, 60.0, 220.0, 30.0, "").clicked {
            // Kept from the installer's main menu (now hidden in the title).
            self.dialogs
                .info("You found an Easter Egg", "Never divide by 0!");
        }
    }
}

fn header(kit: &Kit, text: &str) {
    kit.header(text, ((PAGE_W - 450.0) / 2.0).floor(), 0.0, 450.0, 46.0);
}

fn coming_soon(kit: &mut Kit, title: &str, text: &str) {
    header(kit, title);
    kit.text_center(
        0.0,
        120.0,
        PAGE_W,
        120.0,
        text,
        theme::arial(16.0),
        theme::WHITE,
        Some(700.0),
    );
}

/// A small label/value text row helper.
fn label(kit: &Kit, x: f32, y: f32, text: &str) {
    kit.text_left(x, y, 20.0, text, theme::arial_bold(14.0), theme::WHITE);
}
