//! `FrameGuidancePC`: Type → Play → Path → Download → Patch → Done.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;

use super::dialogs::Icon;
use super::kit::{Btn, Kit};
use super::parts::{self, PatchOptions, PathField, Tx, Worker};
use super::theme;
use super::wizard::{Flow, Nav, Shell};
use crate::core::download::{self, Job, Progress};
use crate::core::oauth::{self, FileType, OAuthError};
use crate::core::{self, elevation, paths, revive};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserType {
    Owner,
    NewPlayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayStyle {
    SteamVr,
    MetaLink,
}

/// What step 4 (Patch) currently shows (the Java `patchDetailMode` + inline views).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PatchView {
    Menu,
    Licence { redirect: bool },
    Steam,
    AfterOAuth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Pending,
    Working,
    Done,
    Failed,
}

const ROWS: [&str; 3] = [
    "Install Revive",
    "Revive injector shortcut",
    "Fix game artwork",
];
const ROW_TIPS: [&str; 3] = [
    "Download and run the Revive installer",
    "Add a desktop shortcut that launches Echo through Revive (fixes 'can't press buttons in-game')",
    "Download the correct Echo artwork into your Meta Horizon store assets",
];
const ROW_REVIVE: usize = 0;
const ROW_SHORTCUT: usize = 1;
const ROW_ARTWORK: usize = 2;

enum Msg {
    Status(String),
    /// Game download finished (Err: dialog title/message; None = cancelled).
    GameDone(Result<(), Option<(String, String)>>),
    LicenceUrl(Result<String, OAuthError>),
    LicenceDownloaded(Result<(), String>),
    Row(usize, Row),
    SteamProgress(String),
    SteamDone(Result<(), String>),
    Consent(SyncSender<bool>),
}

pub struct PcInstall {
    user: Option<UserType>,
    play: Option<PlayStyle>,
    root: String,
    path: PathField,
    dl_button: &'static str,
    view: PatchView,
    just_arrived: bool,
    licence_button: String,
    licence_busy: bool,
    licence_ready: bool,
    patch: PatchOptions,
    steam_checks: [bool; 3],
    steam_rows: [Row; 3],
    steam_progress: Option<String>,
    steam_busy: bool,
    consent_reply: Option<SyncSender<bool>>,
    /// "Start download now?" was answered Yes; started on the next frame.
    pending_download: bool,
    cancel: Arc<AtomicBool>,
    worker: Worker<Msg>,
}

impl Default for PcInstall {
    fn default() -> Self {
        let root = parts::initial_install_root();
        PcInstall {
            user: None,
            play: None,
            path: PathField { text: root.clone() },
            root,
            dl_button: "Start Download",
            view: PatchView::Menu,
            just_arrived: false,
            licence_button: "Authorize with Discord".into(),
            licence_busy: false,
            licence_ready: false,
            patch: PatchOptions::default(),
            steam_checks: [true, true, true],
            steam_rows: [Row::Pending; 3],
            steam_progress: None,
            steam_busy: false,
            consent_reply: None,
            pending_download: false,
            cancel: Arc::new(AtomicBool::new(false)),
            worker: Worker::default(),
        }
    }
}

fn staged_dll() -> PathBuf {
    paths::downloads_dir().join("pnsovr.dll")
}

const CONSENT_KEY: &str = "admin-consent";

impl PcInstall {
    fn installed(&self) -> bool {
        paths::has_echo_install(&self.root)
    }

    fn reset_cancel(&mut self) -> Arc<AtomicBool> {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.cancel.clone()
    }

    /// `commitPathField`: resolve to the install root, save, rewrite the field.
    fn commit_path(&mut self, arena: bool) {
        let input = self.path.text.trim().to_string();
        self.root = if input.is_empty() {
            String::new()
        } else {
            parts::commit_install_path(&input)
        };
        if input.is_empty() {
            crate::core::config::save_install_path("");
        }
        self.path.text = if self.root.is_empty() {
            String::new()
        } else if arena {
            format!("{}/{}", self.root, paths::ARENA_DIR)
        } else {
            self.root.clone()
        };
    }

    fn set_field(&mut self, arena: bool) {
        self.path.text = if arena && !self.root.is_empty() {
            format!("{}/{}", self.root, paths::ARENA_DIR)
        } else {
            self.root.clone()
        };
    }

    fn path_row(&mut self, kit: &mut Kit, cx: f32, y: f32, ind: (f32, f32, f32, f32), arena: bool) {
        let x = ((cx - 440.0) / 2.0).floor();
        let valid = self.installed();
        if self.path.show(kit, "path", x, y, valid, ind, "").commit {
            self.commit_path(arena);
        }
    }

    fn detect_meta(&mut self, sh: &mut Shell) {
        if !cfg!(windows) {
            sh.dialogs.error(
                "Windows only",
                "Meta/Oculus path detection is only available on Windows.",
                Default::default(),
            );
            return;
        }
        let Some(base) = core::platform::oculus_base_path() else {
            sh.dialogs.error(
                "Meta install not found",
                "Could not find a Meta/Oculus installation in the registry. Is the Meta Quest (Oculus) app installed?",
                Default::default(),
            );
            return;
        };
        // Meta games live under <Base>\Software\Software\<app>.
        let sep = if base.ends_with(['\\', '/']) {
            ""
        } else {
            "\\"
        };
        self.root = paths::normalize(&format!("{base}{sep}Software\\Software"));
        self.path.text = self.root.clone();
        crate::core::config::save_install_path(&self.root);
        tracing::info!(
            "PathDetect: Meta base={base} -> install path={} installed={}",
            self.root,
            self.installed()
        );
        if self.installed() {
            sh.tipbox.show_tip("Found your Meta Echo VR install!");
        } else {
            sh.dialogs.info(
                "Echo VR not installed yet",
                &format!(
                    "Nice — we found your Meta install folder and set the path to:\n{}\n\n\
                     Echo VR isn't in that folder yet. You've got two easy options:\n\n  \
                     • Install Echo VR from the Meta Store and launch it once, to use your own licence, or\n  \
                     • Skip that and apply the Licence Patch in the Patch step instead.\n\n\
                     Either way the path is ready — you can continue whenever you like.",
                    self.root
                ),
            );
        }
    }

    // ---- download ----

    fn trigger_download(&mut self, sh: &mut Shell, ctx: &egui::Context) {
        sh.status = "Downloading...".into();
        sh.next_enabled = false;
        sh.start_work();
        self.dl_button = "Cancel Download";
        let cancel = self.reset_cancel();
        let root = self.root.clone();
        self.worker.spawn(ctx, move |tx| {
            let job = Job {
                url: "ready-at-dawn-echo-arena.zip".into(),
                dir: PathBuf::from(&root),
                filename: "ready-at-dawn-echo-arena.zip".into(),
                use_mirror: true,
                fresh: false,
                extract: true,
            };
            let cancelled_or = |e: anyhow::Error, title: String| {
                if core::http::is_cancelled(&e) {
                    None
                } else {
                    Some((title, format!("{e:#}")))
                }
            };
            let downloaded = download::run(&job, &cancel, &mut |p| {
                tx.send(Msg::Status(match p {
                    Progress::Status(s) => s,
                    Progress::Percent(v) => format!("{v:.2}%"),
                    Progress::Extracting => "Extracting...".into(),
                    Progress::Extracted => "Extraction complete".into(),
                }))
            });
            let result = match downloaded {
                Err(e) => Err(cancelled_or(e, "Error while Downloading".into())),
                Ok(_) => {
                    tx.send(Msg::Status("Applying update...".into()));
                    let mut status = |s: String| tx.send(Msg::Status(s));
                    core::pc_update::apply(
                        core::pc_update::PC_MANIFEST_URL,
                        &paths::bin_path(&root),
                        &cancel,
                        &mut status,
                    )
                    .map_err(|e| {
                        let title = core::pc_update::error_title(&e);
                        cancelled_or(e, title)
                    })
                }
            };
            tx.send(Msg::GameDone(result));
        });
    }

    // ---- licence patch ----

    fn start_licence(&mut self, sh: &mut Shell, ctx: &egui::Context, redirect: bool) {
        self.commit_path(true);
        self.licence_busy = true;
        sh.next_enabled = false;
        sh.start_work();
        let adv = if self.patch.advanced {
            oauth::validate_dll_url(&self.patch.url)
        } else {
            None
        };
        if self.patch.advanced && adv.is_none() {
            sh.dialogs.error(
                "Wrong URL provided",
                "Your provided Download Link is wrong. Please check!",
                Default::default(),
            );
            self.licence_failed(sh);
            return;
        }
        self.licence_button = "Please Wait...".into();
        let _ = redirect;
        let cancel = self.reset_cancel();
        if let Some(url) = adv {
            self.download_dll(sh, ctx, url);
        } else if self.licence_ready && staged_dll().exists() {
            // Retry after e.g. a wrong path: reuse this session's download, skip OAuth.
            sh.status = "Using downloaded patch...".into();
            self.install_dll(sh);
        } else {
            self.worker.spawn(ctx, move |tx| {
                let mut status = |s: String| tx.send(Msg::Status(s));
                tx.send(Msg::LicenceUrl(oauth::run(
                    FileType::Dll,
                    &cancel,
                    &mut status,
                )));
            });
        }
    }

    fn download_dll(&mut self, sh: &mut Shell, ctx: &egui::Context, url: String) {
        sh.status = "Downloading patch file...".into();
        let cancel = self.cancel.clone();
        self.worker.spawn(ctx, move |tx| {
            let job = Job {
                url,
                dir: paths::downloads_dir(),
                filename: "pnsovr.dll".into(),
                use_mirror: false,
                fresh: true,
                extract: false,
            };
            let r = download::run(&job, &cancel, &mut |_| {})
                .map(|_| ())
                .map_err(|e| format!("{e:#}"));
            tx.send(Msg::LicenceDownloaded(r));
        });
    }

    /// Copies the staged dll into the install. On failure the staged file is kept so a
    /// Retry reuses it.
    fn install_dll(&mut self, sh: &mut Shell) {
        let staged = staged_dll();
        if !staged.exists() {
            sh.dialogs.error(
                "Download missing",
                "The patch file wasn't downloaded. Please try again.",
                Default::default(),
            );
            return self.licence_failed(sh);
        }
        let bin = paths::bin_path(&self.root);
        if !bin.is_dir() {
            sh.dialogs.error(
                "Wrong path",
                "Couldn't find ready-at-dawn-echo-arena\\bin\\win10 at your path.\nFix the path above and hit Retry — your downloaded patch will be reused.",
                Default::default(),
            );
            return self.licence_failed(sh);
        }
        if let Err(e) = std::fs::copy(&staged, bin.join("pnsovr.dll")) {
            sh.dialogs.error(
                "Couldn't write patch",
                &format!("Couldn't copy the patch into your install folder.\n{e}\n\nTry running the installer as administrator, then hit Retry."),
                Default::default(),
            );
            return self.licence_failed(sh);
        }
        sh.finish_work(true);
        sh.next_enabled = true;
        self.licence_busy = false;
        self.licence_button = "Done".into();
        sh.status = "License patch applied!".into();
        if let PatchView::Licence { redirect: true } = self.view {
            if self.play == Some(PlayStyle::SteamVr) {
                // New player + SteamVR: chain straight into the Steam patch.
                sh.go(Nav::Show(4, 1));
            } else {
                self.view = PatchView::AfterOAuth;
            }
        }
    }

    fn licence_failed(&mut self, sh: &mut Shell) {
        self.licence_button = "Retry".into();
        self.licence_busy = false;
        sh.reset_after_error();
    }

    // ---- steam patch ----

    fn run_steam_chain(&mut self, sh: &mut Shell, ctx: &egui::Context) {
        if !cfg!(windows) {
            sh.dialogs.error(
                "Windows only",
                "Revive is a Windows-only SteamVR shim. These patches can only be applied on Windows.",
                Default::default(),
            );
            return;
        }
        if !self.steam_checks.iter().any(|c| *c) {
            sh.dialogs.error(
                "Nothing selected",
                "Select at least one patch to apply.",
                Default::default(),
            );
            return;
        }
        let exe = paths::exe_path(&self.root);
        if !exe.is_file() {
            sh.dialogs.error(
                "Echo VR not found",
                "echovr.exe was not found at your install path. Download Echo VR first, then apply patches.",
                Default::default(),
            );
            return;
        }
        let checks = self.steam_checks;
        self.steam_rows = [Row::Pending; 3];
        self.steam_busy = true;
        self.steam_progress = Some(" ".into());
        sh.next_enabled = false;
        sh.start_work();
        let cancel = self.reset_cancel();
        self.worker.spawn(ctx, move |tx| {
            let r = steam_chain(&tx, &cancel, checks, &exe);
            tx.send(Msg::SteamDone(r));
        });
    }

    fn apply_msg(&mut self, sh: &mut Shell, ctx: &egui::Context, m: Msg) {
        match m {
            Msg::Status(s) => sh.status = s,
            Msg::GameDone(r) => {
                sh.in_progress = false;
                self.dl_button = "Start Download";
                sh.next_enabled = true;
                match r {
                    Ok(()) => {
                        sh.completed = true;
                        sh.status = "Installation complete!".into();
                    }
                    Err(None) => sh.status = "Ready to download".into(),
                    Err(Some((title, msg))) => {
                        sh.status = "Download failed".into();
                        sh.dialogs.error(&title, &msg, Default::default());
                    }
                }
            }
            Msg::LicenceUrl(Ok(url)) => self.download_dll(sh, ctx, url),
            Msg::LicenceUrl(Err(e)) => {
                if matches!(e, OAuthError::NotInGuild(_)) {
                    if let Some((title, msg)) = e.dialog() {
                        sh.dialogs.options(
                            "join-server",
                            title,
                            &msg,
                            Icon::Warning,
                            &["Join Server", "Close"],
                        );
                    }
                } else if let Some((title, msg)) = e.dialog() {
                    sh.dialogs.error(title, &msg, Default::default());
                }
                self.licence_failed(sh);
            }
            Msg::LicenceDownloaded(Ok(())) => {
                self.licence_ready = true;
                self.install_dll(sh);
            }
            Msg::LicenceDownloaded(Err(e)) => {
                if !e.contains("cancelled") {
                    sh.dialogs
                        .error("Error while Downloading", &e, Default::default());
                }
                self.licence_failed(sh);
            }
            Msg::Row(i, r) => self.steam_rows[i] = r,
            Msg::SteamProgress(s) => {
                sh.status = s.clone();
                self.steam_progress = Some(s);
            }
            Msg::SteamDone(Ok(())) => {
                self.steam_busy = false;
                sh.finish_work(true);
                sh.next_enabled = true;
                self.steam_progress = Some("Revive setup complete!".into());
                sh.status = "Revive setup complete!".into();
            }
            Msg::SteamDone(Err(e)) => {
                self.steam_busy = false;
                sh.next_enabled = true;
                if !e.is_empty() {
                    sh.dialogs
                        .error("Steam Patch Failed", &e, Default::default());
                }
                sh.reset_after_error();
            }
            Msg::Consent(reply) => {
                self.consent_reply = Some(reply);
                sh.dialogs.confirm(
                    CONSENT_KEY,
                    "Administrator rights required",
                    "This step needs administrator rights to write into protected folders\n(applying Revive patches).\n\nStart the privileged helper now? Windows will ask you to confirm.",
                    Icon::Question,
                );
            }
        }
    }

    // ---- views ----

    fn step0(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        let bx = ((cx - Btn::Big.w()) / 2.0).floor();
        kit.header(
            "Do you own Echo VR on your Meta account?",
            ((cx - 450.0) / 2.0).floor(),
            8.0,
            450.0,
            55.0,
        );
        if kit.button(
            "own",
            Btn::Big,
            "I own Echo on Meta",
            18.0,
            bx,
            73.0,
            true,
            "You already own Echo VR",
        ) {
            self.user = Some(UserType::Owner);
            sh.go(Nav::Advance);
        }
        if kit.button(
            "new",
            Btn::Big,
            "I'm a new player",
            18.0,
            bx,
            133.0,
            true,
            "You need to patch Echo VR",
        ) {
            self.user = Some(UserType::NewPlayer);
            sh.go(Nav::Advance);
        }
    }

    fn step1(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        let bx = ((cx - Btn::Big.w()) / 2.0).floor();
        kit.header(
            "How do you play Echo VR?",
            ((cx - 450.0) / 2.0).floor(),
            8.0,
            450.0,
            55.0,
        );
        let steam_tip = "Use this if you launch Echo VR through SteamVR with Revive. A Steam patch will be available in the next step.";
        if kit.button(
            "steamvr",
            Btn::Big,
            "SteamVR (Revive)",
            18.0,
            bx,
            73.0,
            true,
            steam_tip,
        ) {
            self.play = Some(PlayStyle::SteamVr);
            sh.go(Nav::Advance);
        }
        let meta_tip = "Use this if you run Echo directly through the Meta Quest Link app on PC.";
        if kit.button(
            "meta",
            Btn::Big,
            "Meta Link",
            18.0,
            bx,
            133.0,
            true,
            meta_tip,
        ) {
            self.play = Some(PlayStyle::MetaLink);
            sh.go(Nav::Advance);
        }
    }

    fn step2(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        kit.header(
            "Choose your Echo VR install path",
            ((cx - 450.0) / 2.0).floor(),
            5.0,
            450.0,
            55.0,
        );
        let ix = ((cx - 440.0) / 2.0).floor() + 445.0;
        self.path_row(kit, cx, 70.0, (ix, 64.0, 90.0, 34.0), false);
        let bx = ((cx - Btn::Small.w()) / 2.0).floor();
        let tip =
            "Manually choose install folder — picking the game folder or a subfolder works too";
        if kit.button(
            "choose",
            Btn::Small,
            "Choose path",
            11.0,
            bx,
            102.0,
            true,
            tip,
        ) {
            if let Some(p) = parts::choose_folder() {
                self.path.text = p;
                self.commit_path(false);
                sh.go(Nav::Show(3, 0));
            }
        }
        if kit.button(
            "detect",
            Btn::Small,
            "Detect Meta path",
            11.0,
            bx,
            134.0,
            true,
            "Auto-detect where Meta/Oculus installs games",
        ) {
            self.detect_meta(sh);
        }
    }

    fn step3(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        let ctx = kit.ctx();
        kit.header(
            "Download Echo VR client files",
            ((cx - 450.0) / 2.0).floor(),
            5.0,
            450.0,
            55.0,
        );
        let ix = ((cx - 440.0) / 2.0).floor() + 445.0;
        self.path_row(kit, cx, 70.0, (ix, 64.0, 90.0, 34.0), false);
        let bx = ((cx - Btn::Big.w()) / 2.0).floor();
        if kit.button(
            "download",
            Btn::Big,
            self.dl_button,
            18.0,
            bx,
            102.0,
            true,
            "Download Echo VR client files",
        ) {
            if sh.in_progress {
                sh.go(Nav::Show(3, 0)); // routed through the abort confirmation
            } else {
                self.commit_path(false);
                if self.installed() {
                    sh.dialogs.confirm(
                        "overwrite",
                        "Existing Installation Found",
                        "Echo VR is already installed at this path.\n\nOverwrite the existing installation?",
                        Icon::Warning,
                    );
                } else if self.root.is_empty() {
                    sh.dialogs.error(
                        "No Install Path",
                        "Please choose an install path first.",
                        Default::default(),
                    );
                } else {
                    self.trigger_download(sh, &ctx);
                }
            }
        }
        if sh.dialogs.take("overwrite").is_some_and(|a| a.is_yes()) {
            self.trigger_download(sh, &ctx);
        }
    }

    fn step4(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        if sh.sub == 1 {
            self.view = PatchView::Steam;
        }
        match self.view {
            PatchView::Menu => self.patch_menu(sh, kit, cx),
            PatchView::AfterOAuth => self.patch_menu(sh, kit, cx),
            PatchView::Licence { .. } => self.licence_view(sh, kit, cx),
            PatchView::Steam => self.steam_view(sh, kit, cx),
        }
    }

    fn patch_menu(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        let hx = ((cx - 450.0) / 2.0).floor();
        let bx = ((cx - Btn::Middle.w()) / 2.0).floor();
        let owner_menu =
            self.view == PatchView::AfterOAuth || self.user != Some(UserType::NewPlayer);
        if owner_menu {
            kit.header("Optional patches", hx, 8.0, 450.0, 55.0);
            if kit.button(
                "nolicence",
                Btn::Middle,
                "No Licence Patch",
                14.0,
                bx,
                73.0,
                true,
                "Patch Echo VR to skip licence check",
            ) {
                self.open_licence(false);
            }
        } else {
            kit.header("Patch Menu", hx, 8.0, 450.0, 55.0);
            if kit.button(
                "licence",
                Btn::Middle,
                "Licence Patch",
                14.0,
                bx,
                73.0,
                true,
                "Get your licence patch",
            ) {
                self.open_licence(true);
            }
        }
        if kit.button(
            "steam",
            Btn::Middle,
            "Steam Patch (Revive)",
            14.0,
            bx,
            121.0,
            true,
            "Patch Echo VR for Steam/Revive compatibility",
        ) {
            self.view = PatchView::Steam;
        }
        if !sh.in_progress {
            sh.next_enabled = true;
        }
    }

    fn open_licence(&mut self, redirect: bool) {
        self.view = PatchView::Licence { redirect };
        self.set_field(true);
        if !self.licence_busy {
            self.licence_button = if self.patch.advanced {
                "Start Patching"
            } else {
                "Authorize with Discord"
            }
            .into();
        }
    }

    fn back_button(&mut self, sh: &mut Shell, kit: &mut Kit) {
        if kit.button(
            "patch-back",
            Btn::Small,
            "← Back",
            11.0,
            10.0,
            5.0,
            true,
            "Return to patch selection",
        ) {
            sh.go(Nav::Show(4, 0));
        }
    }

    fn licence_view(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        let ctx = kit.ctx();
        self.back_button(sh, kit);
        kit.header(
            "No Licence Patch",
            ((cx - 450.0) / 2.0).floor(),
            4.0,
            450.0,
            42.0,
        );
        let ix = ((cx - 440.0) / 2.0).floor() + 445.0;
        self.path_row(kit, cx, 54.0, (ix, 52.0, 40.0, 28.0), true);
        let bx = ((cx - Btn::Small.w()) / 2.0).floor();
        let tip = "Choose your Echo VR install folder — the game folder or a subfolder works too";
        if kit.button(
            "choose",
            Btn::Small,
            "Choose path",
            11.0,
            bx,
            86.0,
            !self.licence_busy,
            tip,
        ) {
            if let Some(p) = parts::choose_folder() {
                self.path.text = p;
                self.commit_path(true);
            }
        }
        let mut label = std::mem::take(&mut self.licence_button);
        let ev = self.patch.show(
            kit,
            cx,
            119.0,
            &mut label,
            "Authorize with Discord",
            "Start Patching",
            !self.licence_busy,
            |t| oauth::validate_dll_url(t).is_some(),
            "Paste a direct URL to use a custom patch instead of generating one.",
            (
                "Generate your personalized licence patch",
                "Download and apply the patch from your pasted URL",
            ),
        );
        self.licence_button = label;
        if ev.toggled {
            self.licence_ready = false; // a mode switch is a fresh start
        }
        if ev.clicked {
            let redirect = matches!(self.view, PatchView::Licence { redirect: true });
            self.start_licence(sh, &ctx, redirect);
        }
    }

    fn steam_view(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        let ctx = kit.ctx();
        self.back_button(sh, kit);
        kit.header(
            "Steam Patch (Revive)",
            ((cx - 450.0) / 2.0).floor(),
            4.0,
            450.0,
            42.0,
        );
        let (pad, pitch, row_h, glyph_w, gap) = (12.0, 20.0, 20.0, 22.0, 6.0);
        let n = ROWS.len() as f32;
        let (btn_w, btn_h) = Btn::Middle.size();
        let rows_h = (n - 1.0) * pitch + row_h;
        let box_w = (cx - 20.0).min(btn_w.max(300.0) + 2.0 * pad);
        let box_x = ((cx - box_w) / 2.0).floor();
        let box_y = 48.0;
        let box_h = pad + rows_h + gap + btn_h + pad;
        kit.section_box(box_x, box_y, box_w, box_h, 15.0, theme::SECTION_FILL);
        let first = box_y + pad;
        let cb_x = box_x + pad;
        let glyph_x = box_x + box_w - pad - glyph_w;
        let cb_w = glyph_x - cb_x - 6.0;
        for (i, label) in ROWS.iter().enumerate() {
            let y = first + i as f32 * pitch;
            let mut checked = self.steam_checks[i];
            kit.checkbox(
                &format!("row{i}"),
                &mut checked,
                label,
                14.0,
                cb_x,
                y,
                cb_w,
                row_h,
                false,
                !self.steam_busy,
                ROW_TIPS[i],
            );
            self.steam_checks[i] = checked;
            let gx = glyph_x + glyph_w - 16.0;
            match self.steam_rows[i] {
                Row::Pending => kit.text_center(
                    glyph_x,
                    y,
                    glyph_w,
                    row_h,
                    "○",
                    theme::arial_bold(14.0),
                    theme::LIGHT_GRAY,
                    None,
                ),
                Row::Working => kit.text_center(
                    glyph_x,
                    y,
                    glyph_w,
                    row_h,
                    "●",
                    theme::arial_bold(14.0),
                    theme::CURRENT_GREEN,
                    None,
                ),
                Row::Done => kit.mark(true, theme::DONE_GREEN, 16.0, gx, y + 2.0),
                Row::Failed => kit.mark(false, theme::MARK_BAD, 16.0, gx, y + 2.0),
            }
        }
        let by = first + rows_h + gap;
        if kit.button(
            "steam-start",
            Btn::Middle,
            "Install & Configure",
            14.0,
            box_x + ((box_w - btn_w) / 2.0).floor(),
            by,
            !self.steam_busy,
            "Run the selected Revive patches",
        ) {
            self.run_steam_chain(sh, &ctx);
        }
        if let Some(p) = &self.steam_progress {
            let h = parts::label_height(kit, 13.0).min(18.0);
            kit.special_label(
                box_x,
                box_y + box_h + 4.0,
                box_w,
                h,
                p,
                13.0,
                theme::LABEL_BG,
                theme::WHITE,
            );
        }
    }

    fn step5(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        parts::done_texts(
            kit,
            cx,
            20.0,
            "You're all set!",
            70.0,
            "Echo VR is ready to play.",
        );
        let bx = ((cx - Btn::Big.w()) / 2.0).floor();
        if kit.button(
            "shortcut",
            Btn::Big,
            "Add Desktop Shortcut",
            18.0,
            bx,
            115.0,
            true,
            "Create a shortcut to Echo VR on your desktop",
        ) {
            if !self.installed() {
                sh.dialogs.error(
                    "No Install Path",
                    "Echo VR is not installed. Please download and install Echo VR first.",
                    Default::default(),
                );
            } else {
                let exe = paths::exe_path(&self.root);
                let bin = paths::bin_path(&self.root);
                match core::platform::create_shortcut("Echo VR", &exe, None, Some(&bin), Some(&exe))
                {
                    Ok(()) => sh.dialogs.info("Done", "Desktop shortcut created!"),
                    Err(e) => sh.dialogs.error(
                        "Couldn't create shortcut",
                        &format!("{e:#}"),
                        Default::default(),
                    ),
                }
            }
        }
        if kit.button(
            "open",
            Btn::Big,
            "Open Install Folder",
            18.0,
            bx,
            180.0,
            true,
            "Open the Echo VR install folder in file explorer",
        ) {
            if self.root.is_empty() {
                sh.dialogs.error(
                    "No Install Path",
                    "Echo VR is not installed. Please download and install Echo VR first.",
                    Default::default(),
                );
            } else if let Err(e) = core::platform::open_folder(&paths::bin_path(&self.root)) {
                sh.dialogs.error(
                    "Couldn't open folder",
                    &format!("{e:#}"),
                    Default::default(),
                );
            }
        }
    }
}

/// The Revive chain, on a worker thread. `Err("")` means a failure already shown.
fn steam_chain(
    tx: &Tx<Msg>,
    cancel: &AtomicBool,
    checks: [bool; 3],
    exe: &std::path::Path,
) -> Result<(), String> {
    let mut consent = || {
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        tx.send(Msg::Consent(reply_tx));
        reply_rx.recv().unwrap_or(false)
    };
    tracing::info!(
        "SteamPatch: chain start exe={} selected={checks:?}",
        exe.display()
    );

    if checks[ROW_REVIVE] {
        tx.send(Msg::Row(ROW_REVIVE, Row::Working));
        tx.send(Msg::SteamProgress("Installing Revive...".into()));
        let result = revive::download_installer(cancel, &mut |p| {
            if let Progress::Percent(v) = p {
                tx.send(Msg::SteamProgress(format!("Downloading Revive... {v:.0}%")));
            }
        })
        .and_then(|installer| {
            tx.send(Msg::SteamProgress("Installing Revive...".into()));
            elevation::run_revive_installer(&installer, &mut consent)
        });
        match result {
            Ok(code) => tracing::info!("SteamPatch: installer exited with {code}"),
            Err(e) => {
                tx.send(Msg::Row(ROW_REVIVE, Row::Failed));
                return Err(format!("Installing Revive failed: {e:#}"));
            }
        }
        // The installer's exit code is unreliable; the injector's presence is the check.
        if revive::wait_for_revive_dir(std::time::Duration::from_secs(8)).is_none() {
            tx.send(Msg::Row(ROW_REVIVE, Row::Failed));
            return Err("Revive does not appear to be installed (was the installer cancelled?). Re-run, or install Revive manually, then try again.".into());
        }
        tx.send(Msg::Row(ROW_REVIVE, Row::Done));
    }

    if checks[ROW_SHORTCUT] {
        let Some(dir) = revive::find_revive_dir() else {
            tx.send(Msg::Row(ROW_SHORTCUT, Row::Failed));
            return Err("Revive is not installed. Tick 'Install Revive' and run again, or install Revive manually first.".into());
        };
        tx.send(Msg::Row(ROW_SHORTCUT, Row::Working));
        tx.send(Msg::SteamProgress("Creating Revive shortcut...".into()));
        if let Err(e) = revive::create_injector_shortcut(&dir, exe) {
            tx.send(Msg::Row(ROW_SHORTCUT, Row::Failed));
            return Err(format!("Creating the shortcut failed: {e:#}"));
        }
        tx.send(Msg::Row(ROW_SHORTCUT, Row::Done));
    }

    if checks[ROW_ARTWORK] {
        tx.send(Msg::Row(ROW_ARTWORK, Row::Working));
        tx.send(Msg::SteamProgress("Installing game artwork...".into()));
        if let Err(e) = elevation::install_artwork(&mut consent) {
            tx.send(Msg::Row(ROW_ARTWORK, Row::Failed));
            return Err(format!("Installing the artwork failed: {e:#}"));
        }
        tx.send(Msg::Row(ROW_ARTWORK, Row::Done));
    }
    tracing::info!("SteamPatch: chain complete");
    Ok(())
}

impl Flow for PcInstall {
    fn background(&self) -> &'static str {
        "EchoArena.jpg"
    }

    fn step_count(&self) -> usize {
        6
    }

    fn chip(&self, step: usize) -> &'static str {
        ["Type", "Play", "Path", "Download", "Patch", "Done"][step]
    }

    fn substep_count(&self, step: usize) -> usize {
        if step == 4 && self.play == Some(PlayStyle::SteamVr) {
            2
        } else {
            1
        }
    }

    fn substep_name(&self, step: usize, sub: usize) -> String {
        match (step, sub) {
            (0, _) => "Choose Type",
            (1, _) => "How do you play?",
            (2, _) => "Choose Path",
            (3, _) => "Download",
            (4, 1) => "Steam Patch",
            (4, _) if self.user == Some(UserType::NewPlayer) => "Authorize & Patch",
            (4, _) => "Optional Patches",
            _ => "All Done",
        }
        .into()
    }

    fn status_text(&self, _sh: &Shell, step: usize, sub: usize) -> String {
        match (step, sub) {
            (0, _) => "Choose your player type",
            (1, _) => "How do you launch Echo VR?",
            (2, _) => "Choose your Echo VR install path",
            (3, _) => "Ready to download",
            (4, 1) => "Apply Steam patch for Revive compatibility",
            (4, _) if self.user == Some(UserType::NewPlayer) => {
                "Authorize with Discord to generate your patch"
            }
            (4, _) => "Apply optional patches",
            _ => "Echo VR installation complete!",
        }
        .into()
    }

    fn enter(&mut self, _sh: &mut Shell, step: usize, sub: usize, from: usize) -> (usize, usize) {
        self.just_arrived = step == 4 && from != 4;
        // Owner + SteamVR arriving forward from Download goes straight to the Steam patch.
        if step == 4
            && sub == 0
            && from == 3
            && self.user == Some(UserType::Owner)
            && self.play == Some(PlayStyle::SteamVr)
        {
            return (4, 1);
        }
        (step, sub)
    }

    fn entered(&mut self, sh: &mut Shell) {
        self.view = PatchView::Menu;
        self.licence_busy = false;
        self.steam_busy = false;
        self.steam_progress = None;
        self.steam_rows = [Row::Pending; 3];
        match sh.step {
            2 | 3 => {
                self.dl_button = "Start Download";
                self.set_field(false);
            }
            4 if sh.sub == 0 && self.user == Some(UserType::NewPlayer) && self.just_arrived => {
                self.open_licence(true);
            }
            5 => sh.next_enabled = true,
            _ => {}
        }
    }

    fn can_advance(&mut self, sh: &mut Shell) -> bool {
        match sh.step {
            0 if self.user.is_none() => {
                sh.dialogs.warning(
                    "No Player Type Selected",
                    "Please select whether you own Echo VR or are a new player before continuing.",
                );
                false
            }
            1 if self.play.is_none() => {
                sh.dialogs.warning(
                    "No Playstyle Selected",
                    "Please select how you launch Echo VR (SteamVR or Meta Link) before continuing.",
                );
                false
            }
            2 => {
                self.commit_path(false);
                true
            }
            3 if !sh.in_progress && !self.installed() => {
                sh.dialogs.confirm(
                    "start-download",
                    "Echo VR not found",
                    "Echo VR hasn't been installed yet.\n\nStart download now?",
                    Icon::Question,
                );
                false
            }
            _ => true,
        }
    }

    fn chip_click(&mut self, sh: &mut Shell, step: usize) -> bool {
        if step == 4 && !self.installed() {
            sh.dialogs.confirm(
                "chip-not-installed",
                "Echo not installed",
                "Echo VR needs to be installed first.\n\nGo to download step?",
                Icon::Question,
            );
            return true;
        }
        false
    }

    fn poll(&mut self, sh: &mut Shell) {
        if let Some(a) = sh.dialogs.take("chip-not-installed") {
            sh.go(if a.is_yes() {
                Nav::Show(3, 0)
            } else {
                Nav::Show(4, 0)
            });
        }
        if let Some(a) = sh.dialogs.take("start-download") {
            if a.is_yes() {
                self.pending_download = true;
            } else {
                sh.go(Nav::Show(4, 0));
            }
        }
        if sh.dialogs.take("join-server").is_some_and(|a| a.is_yes()) {
            core::platform::open_url(oauth::INVITE_URL);
        }
        if let Some(a) = sh.dialogs.take(CONSENT_KEY) {
            if let Some(reply) = self.consent_reply.take() {
                let _ = reply.send(a.is_yes());
            }
        }
    }

    fn abort(&mut self, sh: &mut Shell) {
        self.cancel.store(true, Ordering::Relaxed);
        self.dl_button = "Start Download";
        if sh.step == 3 {
            sh.status = "Ready to download".into();
            sh.next_enabled = true;
        }
    }

    fn content(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        let ctx = kit.ctx();
        for m in self.worker.drain() {
            self.apply_msg(sh, &ctx, m);
        }
        if std::mem::take(&mut self.pending_download) {
            self.trigger_download(sh, &ctx);
        }
        match sh.step {
            0 => self.step0(sh, kit, cx),
            1 => self.step1(sh, kit, cx),
            2 => self.step2(sh, kit, cx),
            3 => self.step3(sh, kit, cx),
            4 => self.step4(sh, kit, cx),
            _ => self.step5(sh, kit, cx),
        }
    }
}
