//! The launcher (root window): the design's icon rail, the status bar and pages over the
//! purple backdrop. See `ui/design.rs` and `ui/style.rs` for the widgets. Installing,
//! patching, SteamVR setup and the Quest all run inline (`setup.rs`).

mod play;
mod server_info;
mod settings;
mod setup;
mod versions;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::design::{self, dz, Dr, RailIcon};
use super::dialogs::DialogHost;
use super::kit::Kit;
use super::parts::{QuestConn, Worker};
use super::style::{self, Icon};
use super::tipbox::Clippy;
use crate::core::adb::devices::Status;
use crate::core::error::UiError;
use crate::core::launcher::catalog::{Catalog, Platform, VersionEntry};
use crate::core::launcher::feed;
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
const RAIL: f32 = dz(91.0);
/// Left edge and width of page content.
const X0: f32 = dz(138.0);
const CW: f32 = W - X0 - dz(48.0);
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
    /// The community modules (placeholders for now).
    Spark,
    EchoVrce,
    Community,
    Settings,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Play => "Play",
            Page::Versions => "Versions",
            Page::Mods => "Mods",
            Page::Servers => "Servers",
            Page::Spark => "Spark",
            Page::EchoVrce => "EchoVRCE",
            Page::Community => "Community",
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
    FeedStatus(Option<feed::Servers>),
    FeedNews(Option<feed::News>),
    /// A feed image by file name (`None`: it couldn't be loaded).
    FeedImage(String, Option<image::RgbaImage>),
}

/// The Play page's feed (SERVER INFO and Community News) and its images.
#[derive(Default)]
struct Feed {
    status: Option<feed::Servers>,
    news: Option<feed::News>,
    /// A fetch failed and there is nothing to show.
    status_failed: bool,
    news_failed: bool,
    status_at: Option<std::time::Instant>,
    news_at: Option<std::time::Instant>,
    status_loading: bool,
    news_loading: bool,
    textures: HashMap<String, egui::TextureHandle>,
    /// Images being downloaded, or that failed (retried when the data changes).
    images_pending: std::collections::HashSet<String>,
}

impl Feed {
    const STATUS_EVERY: std::time::Duration = std::time::Duration::from_secs(60);
    const NEWS_EVERY: std::time::Duration = std::time::Duration::from_secs(600);

    /// The image file names the news refers to.
    fn wanted(&self) -> Vec<String> {
        self.news
            .iter()
            .flat_map(|n| [&n.slots.main, &n.slots.community])
            .flatten()
            .filter_map(|i| i.image.clone())
            .collect()
    }

    /// Status and news have arrived, with every image they refer to.
    #[cfg(test)]
    fn ready(&self) -> bool {
        self.status.is_some()
            && self.news.is_some()
            && self.wanted().iter().all(|n| self.textures.contains_key(n))
    }

    /// The texture of a feed image, once downloaded.
    fn texture(&self, name: Option<&str>) -> Option<&egui::TextureHandle> {
        self.textures.get(name?)
    }
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
    /// A version that is not installed is selected.
    NotInstalled,
    /// That version is being installed.
    Installing,
    /// The first-run setup card.
    Setup,
    /// Its second step (how you play).
    SetupHeadset,
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
    /// Snapshots: fetch the real feed instead of the made-up one.
    pub feed_live: bool,
    pub snap_variant: Option<SnapVariant>,
    /// The variant applied last (`Some(None)`: the plain page).
    applied_variant: Option<Option<SnapVariant>>,
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
    feed: Feed,
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
    fn apply_snap_variant(&mut self) {
        if self.applied_variant == Some(self.snap_variant) {
            return;
        }
        self.applied_variant = Some(self.snap_variant);
        self.jobs.clear();
        self.overlay = None;
        self.state.owner = Some(true);
        self.state.selected = Some("pc-latest".into());
        self.play_platform = Platform::Pc;
        // A connected headset with Echo VR on it, as in the design concept.
        self.quest_conn.status = Some(Status::Ready);
        self.quest_info = Some(QuestInfo {
            device: Some("Meta Quest 3 (2G0YC5ZF8R0123)".into()),
            installed: true,
            marker: Some(crate::core::quest_update::Marker {
                base_apk: Some("r15_26-06-23.apk".into()),
                ..Default::default()
            }),
        });
        self.update_note.clear();
        match self.snap_variant {
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
            Some(SnapVariant::QuestSide) => self.play_platform = Platform::Quest,
            // The concept's orange "!" on CHECK FOR UPDATES.
            None => {
                self.update_note
                    .insert("pc-latest".into(), "Last update failed".into());
            }
        }
    }

    fn refresh_catalog(&mut self, ctx: &egui::Context) {
        self.catalog_loading = true;
        self.worker
            .spawn(ctx, |tx| tx.send(Msg::Catalog(Catalog::load())));
    }

    /// Fetches SERVER INFO every minute and the news every ten, plus any image they
    /// refer to that isn't loaded yet. Snapshots use made-up data.
    fn poll_feed(&mut self, ctx: &egui::Context) {
        if self.demo && !self.feed_live {
            if self.feed.status.is_none() {
                self.feed.status = Some(feed::mock_servers());
                self.feed.news = Some(feed::mock_news());
            }
            return;
        }
        let due = |at: Option<std::time::Instant>, every| at.is_none_or(|t| t.elapsed() >= every);
        if !self.feed.status_loading && due(self.feed.status_at, Feed::STATUS_EVERY) {
            self.feed.status_loading = true;
            self.feed.status_at = Some(std::time::Instant::now());
            self.worker.spawn(ctx, |tx| {
                let s = feed::fetch_servers()
                    .inspect_err(|e| tracing::warn!("server info unavailable: {e:#}"))
                    .ok();
                tx.send(Msg::FeedStatus(s));
            });
        }
        if !self.feed.news_loading && due(self.feed.news_at, Feed::NEWS_EVERY) {
            self.feed.news_loading = true;
            self.feed.news_at = Some(std::time::Instant::now());
            self.worker.spawn(ctx, |tx| {
                let n = feed::fetch_news()
                    .inspect_err(|e| tracing::warn!("community news unavailable: {e:#}"))
                    .ok();
                tx.send(Msg::FeedNews(n));
            });
        }
        let wanted = self.feed.wanted();
        self.feed.textures.retain(|name, _| wanted.contains(name));
        for name in wanted {
            if self.feed.textures.contains_key(&name) || self.feed.images_pending.contains(&name) {
                continue;
            }
            self.feed.images_pending.insert(name.clone());
            self.worker.spawn(ctx, move |tx| {
                let img = feed::fetch_image(&name)
                    .inspect_err(|e| tracing::warn!("feed image {name}: {e:#}"))
                    .ok();
                tx.send(Msg::FeedImage(name, img));
            });
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(10));
    }

    /// Snapshots: the live feed has fully arrived (or failed for good).
    #[cfg(test)]
    pub fn feed_settled(&self) -> bool {
        self.feed.ready() || self.feed.status_failed || self.feed.news_failed
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
                Msg::FeedStatus(s) => {
                    self.feed.status_loading = false;
                    self.feed.status_failed = s.is_none() && self.feed.status.is_none();
                    if s.is_some() {
                        self.feed.status = s;
                        self.feed.images_pending.clear();
                    }
                }
                Msg::FeedNews(n) => {
                    self.feed.news_loading = false;
                    self.feed.news_failed = n.is_none() && self.feed.news.is_none();
                    if n.is_some() {
                        self.feed.news = n;
                        self.feed.images_pending.clear();
                    }
                }
                Msg::FeedImage(name, img) => {
                    if let Some(img) = img {
                        let size = [img.width() as usize, img.height() as usize];
                        let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                        let options = egui::TextureOptions::LINEAR
                            .with_mipmap_mode(Some(egui::TextureFilter::Linear));
                        let tex = ctx.load_texture(format!("feed:{name}"), color, options);
                        self.feed.images_pending.remove(&name);
                        self.feed.textures.insert(name, tex);
                    }
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
        self.poll_feed(&ctx);
        if self.demo {
            self.apply_snap_variant();
        }
        // Track how long the game has been running.
        match (self.game().is_running(), self.game_since) {
            (true, None) => self.game_since = Some(std::time::Instant::now()),
            (false, Some(_)) => self.game_since = None,
            _ => {}
        }

        kit.image("main_background.jpg", 0.0, 0.0, W, H);

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
            p @ (Page::Spark | Page::EchoVrce | Page::Community) => empty_state(
                kit,
                Icon::Info,
                p.title(),
                &format!("The {} module is coming to the launcher soon.", p.title()),
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

    /// The design's rail: pages, a divider, the community modules, and Settings at the
    /// bottom. Positions are the icons' centres in design pixels.
    fn rail(&mut self, kit: &mut Kit) {
        kit.image("left_sidebar.jpg", 0.0, 0.0, RAIL, H);
        let items = [
            (Page::Play, RailIcon::Image("icon_play.png", 24.0), 232.5),
            (
                Page::Versions,
                RailIcon::Vector(Icon::Download, 30.0),
                335.0,
            ),
            (Page::Mods, RailIcon::Vector(Icon::Mods, 30.0), 437.0),
            (Page::Servers, RailIcon::Vector(Icon::Globe, 33.0), 538.0),
            (Page::Spark, RailIcon::Image("icon_spark.png", 29.0), 692.0),
            (
                Page::EchoVrce,
                RailIcon::Image("icon_echovrce.png", 38.0),
                753.0,
            ),
            (
                Page::Community,
                RailIcon::Image("icon_community.png", 38.0),
                814.0,
            ),
            (Page::Settings, RailIcon::Vector(Icon::Gear, 34.0), 1035.0),
        ];
        for (page, icon, cy) in items {
            if kit.rail_item(
                &format!("rail-{}", page.title()),
                icon,
                cy,
                self.page == page,
                page.title(),
            ) {
                self.page = page;
            }
        }
        kit.ui.painter().rect_filled(
            kit.drect(Dr::new(17.0, 624.0, 59.0, 3.0)),
            dz(1.5),
            egui::Color32::from_rgb(142, 144, 143),
        );
    }

    /// The status bar: the Quest chip (click to check the connection) and what the game
    /// or the running job is doing. On Play it spans the main column only.
    fn top_bar(&mut self, kit: &mut Kit, ctx: &egui::Context) {
        let bar = if self.page == Page::Play {
            Dr::new(138.0, 15.0, 1144.0, 43.0)
        } else {
            Dr::new(138.0, 15.0, 1734.0, 43.0)
        };
        kit.ui
            .painter()
            .rect_filled(kit.drect(bar), dz(6.0), design::BAR);

        let (qtext, qcolor) = match (self.quest_conn.checking, self.quest_conn.status) {
            (true, _) => ("Quest: checking...", design::QUEST_OFF),
            (_, Some(Status::Ready)) => ("Quest: connected", design::QUEST_ON),
            (_, Some(Status::Unauthorized)) => ("Quest: allow this PC", design::QUEST_WARN),
            (_, Some(Status::Ambiguous)) => ("Quest: pick a device", design::QUEST_WARN),
            (_, Some(Status::None)) => ("Quest: not connected", design::QUEST_OFF),
            (_, None) => ("Quest: not checked", design::QUEST_OFF),
        };
        let g = kit.spaced_galley(
            &qtext.to_uppercase(),
            design::din(12.0),
            design::TEXT,
            dz(0.5),
            false,
        );
        let chip = Dr::new(149.0, 23.0, g.size().x / dz(1.0) + 20.0, 27.0);
        let r = kit.drect(chip);
        kit.ui.painter().rect_filled(r, dz(4.0), qcolor);
        kit.ui
            .painter()
            .galley(r.center() - g.size() / 2.0, g, design::TEXT);
        if kit
            .hot(
                "quest-chip",
                r,
                !self.quest_conn.checking,
                "Check the USB connection to your Quest",
            )
            .0
            .clicked
        {
            self.check_quest(ctx, true);
        }

        let game = self.game();
        let busy = self.any_job() || self.quest_busy || self.quest_conn.checking;
        let status = if let Some(j) = self.jobs.values().next() {
            match j.fraction {
                Some(f) => format!("{}   ·   {:.0}%", j.title, f * 100.0),
                None => format!("{}   ·   {}", j.title, j.label),
            }
        } else if let (true, Some(since)) = (game.is_running(), self.game_since) {
            let mins = since.elapsed().as_secs() / 60;
            ctx.request_repaint_after(std::time::Duration::from_secs(20));
            if mins == 0 {
                game.label()
            } else {
                format!("{}   ·   {mins} min", game.label())
            }
        } else {
            game.label()
        };
        let color = if game.is_running() {
            design::QUEST_ON
        } else if busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
            style::mix(design::GREY, design::TEXT, style::pulse(ctx))
        } else {
            design::GREY
        };
        let g = kit.spaced_galley(
            &status.to_uppercase(),
            design::din(13.8),
            color,
            dz(0.5),
            false,
        );
        let x = dz(chip.right() + 12.0);
        let y = dz(bar.y + bar.h / 2.0) - g.size().y / 2.0;
        let max_w = dz(bar.right() - 12.0) - x;
        kit.clipped(x, 0.0, max_w, dz(bar.bottom()), |kit| {
            kit.put(x, y, g);
        });

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
    kit.image("card_bg.png", x, y, w, h);
    kit.gradient_frame(
        kit.rect(x, y, w, h),
        dz(6.0),
        dz(3.0),
        design::RIM_TOP,
        design::RIM_BOTTOM,
    );
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
