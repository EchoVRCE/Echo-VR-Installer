//! `FrameGuidanceQuest`: Type → Download → Install → Done.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::dialogs::Icon;
use super::kit::{Btn, Kit};
use super::parts::{self, PatchOptions, QuestConn, Tx, Worker};
use super::wizard::{Flow, Shell};
use crate::core::download::{self, Job, Progress};
use crate::core::error::UiError;
use crate::core::manifest::Manifest;
use crate::core::oauth::{self, FileType, OAuthError};
use crate::core::{adb, paths, quest_install, quest_update};

/// Fallback only: the real name comes from the manifest's `BASE_APK` header.
const FALLBACK_APK: &str = "echo_quest_16-07-2026.001.apk";
const DATA_ZIP: &str = "_data.zip";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UserType {
    Owner,
    NewPlayer,
}

enum Which {
    Apk,
    Data,
}

enum Msg {
    Status(String),
    Manifest(Option<Manifest>),
    Progress(Which, String),
    ApkDone { patched: bool },
    Downloaded,
    Failed(UiError),
    OAuthFailed(OAuthError),
    Cancelled,
    InstallLabel(String),
    InstallDone(Result<(), UiError>),
}

pub struct QuestInstall {
    user: Option<UserType>,
    manifest: Option<Manifest>,
    manifest_fetched: bool,
    apk_label: String,
    data_label: String,
    install_label: String,
    button: String,
    patch: PatchOptions,
    /// A patched APK finished downloading in this session -- Retry may reuse it.
    patched_ready: bool,
    patched: bool,
    conn: QuestConn,
    conn_started: bool,
    cancel: Arc<AtomicBool>,
    worker: Worker<Msg>,
    /// Kept while the install confirmation is open, to start the worker afterwards.
    ctx: Option<egui::Context>,
}

impl Default for QuestInstall {
    fn default() -> Self {
        QuestInstall {
            user: None,
            manifest: None,
            manifest_fetched: false,
            apk_label: "Not started".into(),
            data_label: "Not started".into(),
            install_label: "Not started yet".into(),
            button: String::new(),
            patch: PatchOptions::default(),
            patched_ready: false,
            patched: false,
            conn: QuestConn::default(),
            conn_started: false,
            cancel: Arc::new(AtomicBool::new(false)),
            worker: Worker::default(),
            ctx: None,
        }
    }
}

fn dir() -> PathBuf {
    paths::downloads_dir()
}

impl QuestInstall {
    fn apk_name(&self) -> String {
        self.manifest
            .as_ref()
            .and_then(|m| m.base_apk_name.clone())
            .unwrap_or_else(|| FALLBACK_APK.into())
    }

    fn reset_cancel(&mut self) -> Arc<AtomicBool> {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.cancel.clone()
    }

    /// The manifest names the APK, so it must be known before any download starts (the
    /// Java wizard fetched it in parallel and could download the fallback, then look for
    /// the manifest's name at install time).
    fn fetch_manifest(tx: &Tx<Msg>) -> Option<Manifest> {
        tx.send(Msg::Status("Checking for the latest version...".into()));
        let m = Manifest::fetch(quest_update::QUEST_MANIFEST_URL)
            .map_err(|e| tracing::warn!("quest manifest: {e:#}"))
            .ok();
        tx.send(Msg::Manifest(m.clone()));
        m
    }

    fn download(
        tx: &Tx<Msg>,
        cancel: &AtomicBool,
        url: &str,
        name: &str,
        mirror: bool,
        fresh: bool,
        which: fn() -> Which,
    ) -> Result<(), ()> {
        let job = Job {
            url: url.into(),
            dir: dir(),
            filename: name.into(),
            use_mirror: mirror,
            fresh,
            extract: false,
        };
        let r = download::run(&job, cancel, &mut |p| match p {
            Progress::Percent(v) => tx.send(Msg::Progress(which(), format!("{v:.2}%"))),
            Progress::Status(s) => tx.send(Msg::Progress(which(), s)),
            _ => {}
        });
        match r {
            Ok(_) => Ok(()),
            Err(e) if crate::core::http::is_cancelled(&e) => {
                tx.send(Msg::Cancelled);
                Err(())
            }
            Err(e) => {
                tx.send(Msg::Failed(UiError::new(
                    "Error while Downloading",
                    format!("{e:#}"),
                )));
                Err(())
            }
        }
    }

    fn start_owner_download(&mut self, sh: &mut Shell, ctx: &egui::Context) {
        sh.status = "Downloading...".into();
        sh.next_enabled = false;
        sh.start_work();
        self.button = "Cancel Download".into();
        let cancel = self.reset_cancel();
        let known = self.manifest_fetched.then(|| self.manifest.clone());
        self.worker.spawn(ctx, move |tx| {
            let m = match known {
                Some(m) => m,
                None => Self::fetch_manifest(&tx),
            };
            let apk = m
                .and_then(|m| m.base_apk_name)
                .unwrap_or_else(|| FALLBACK_APK.into());
            tx.send(Msg::Status("Downloading...".into()));
            let (tx2, cancel2) = (tx.clone(), cancel.clone());
            let data = std::thread::spawn(move || {
                Self::download(&tx2, &cancel2, DATA_ZIP, DATA_ZIP, true, false, || {
                    Which::Data
                })
            });
            let apk_ok = Self::download(&tx, &cancel, &apk, &apk, true, false, || Which::Apk);
            if apk_ok.is_err() {
                cancel.store(true, Ordering::Relaxed);
            }
            let data_ok = data.join().unwrap_or(Err(()));
            if apk_ok.is_ok() {
                tx.send(Msg::ApkDone { patched: false });
            }
            if apk_ok.is_ok() && data_ok.is_ok() {
                tx.send(Msg::Downloaded);
            }
        });
    }

    /// New player: a pasted URL bypasses OAuth; a patched APK downloaded earlier in this
    /// session is reused on Retry; otherwise the Discord flow runs.
    fn start_patched_download(&mut self, sh: &mut Shell, ctx: &egui::Context) {
        sh.next_enabled = false;
        sh.start_work();
        let adv_url = if self.patch.advanced {
            oauth::validate_apk_url(&self.patch.url)
        } else {
            None
        };
        if self.patch.advanced && adv_url.is_none() {
            sh.dialogs.error(
                "Wrong URL provided",
                "Your provided Download Link is wrong. Please check!",
                Default::default(),
            );
            self.button = "Retry".into();
            sh.reset_after_error();
            return;
        }
        self.button = "Please Wait...".into();
        let cancel = self.reset_cancel();
        let known = self.manifest_fetched.then(|| self.manifest.clone());
        let reuse = self.patched_ready && dir().join(self.apk_name()).exists();
        if reuse {
            sh.status = "Using downloaded patch...".into();
        } else if adv_url.is_some() {
            sh.status = "Downloading patched APK from link...".into();
        }
        self.worker.spawn(ctx, move |tx| {
            let m = match known {
                Some(m) => m,
                None => Self::fetch_manifest(&tx),
            };
            let apk = m
                .and_then(|m| m.base_apk_name)
                .unwrap_or_else(|| FALLBACK_APK.into());
            if !reuse {
                let url = match adv_url {
                    Some(u) => u,
                    None => {
                        let mut status = |s: String| tx.send(Msg::Status(s));
                        match oauth::run(FileType::Apk, &cancel, &mut status) {
                            Ok(u) => u,
                            Err(e) => {
                                tx.send(Msg::OAuthFailed(e));
                                return;
                            }
                        }
                    }
                };
                tx.send(Msg::Status("Downloading patched APK...".into()));
                if Self::download(&tx, &cancel, &url, &apk, false, true, || Which::Apk).is_err() {
                    return;
                }
            }
            tx.send(Msg::ApkDone { patched: true });
            if Self::download(&tx, &cancel, DATA_ZIP, DATA_ZIP, true, false, || {
                Which::Data
            })
            .is_ok()
            {
                tx.send(Msg::Downloaded);
            }
        });
    }

    fn start_install(&mut self, sh: &mut Shell, ctx: &egui::Context) {
        sh.next_enabled = false;
        sh.start_work();
        self.install_label = "Installation started! Wait!".into();
        let apk = self.apk_name();
        let manifest = self.manifest.clone();
        let patched = self.patched;
        let cancel = self.reset_cancel();
        self.worker.spawn(ctx, move |tx| {
            let mut label = |s: String| tx.send(Msg::InstallLabel(s));
            let r = quest_install::install(&dir(), &apk, DATA_ZIP, &mut label);
            if let Err(e) = r {
                tx.send(Msg::InstallDone(Err(UiError::from_anyhow(
                    &e,
                    "Installation Failed",
                ))));
                return;
            }
            // Record which base version this install is, before the update runs, so a
            // failed update still leaves a correct marker.
            let installed_sha = download::sha256_file(&dir().join(&apk)).ok();
            quest_update::write_marker_after_install(
                Some(apk.clone()),
                manifest.as_ref().and_then(|m| m.base_apk_sha.clone()),
                installed_sha,
                patched,
            );
            if let Some(m) = manifest {
                label("Applying update...".into());
                if let Err(e) = quest_update::apply(&m, &cancel, &mut label) {
                    // The install itself succeeded; the update wizard can retry.
                    tracing::warn!("post-install update failed: {e:#}");
                    if !crate::core::http::is_cancelled(&e) {
                        let ui = UiError::from_anyhow(&e, "Update Failed");
                        tx.send(Msg::Failed(ui));
                    }
                }
            }
            let _ = std::fs::remove_file(dir().join(DATA_ZIP));
            adb::exec(&["kill-server"]);
            tx.send(Msg::InstallDone(Ok(())));
        });
    }

    fn apply_msg(&mut self, sh: &mut Shell, m: Msg) {
        match m {
            Msg::Status(s) => sh.status = s,
            Msg::Manifest(m) => {
                self.manifest_fetched = true;
                self.manifest = m;
            }
            Msg::Progress(Which::Apk, s) => self.apk_label = s,
            Msg::Progress(Which::Data, s) => self.data_label = s,
            Msg::ApkDone { patched } => {
                self.apk_label = "100.00%".into();
                self.patched = patched;
                if patched {
                    self.patched_ready = true;
                }
            }
            Msg::Downloaded => {
                self.data_label = "100.00%".into();
                sh.finish_work(true);
                sh.next_enabled = true;
                sh.status = "Complete".into();
                self.button = if self.user == Some(UserType::Owner) {
                    "Start Download".into()
                } else {
                    "Done".into()
                };
            }
            Msg::Failed(e) => {
                sh.dialogs.error_ui(&e);
                if sh.step == 1 {
                    self.after_download_failure(sh);
                }
            }
            Msg::OAuthFailed(e) => {
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
                self.after_download_failure(sh);
            }
            Msg::Cancelled => {
                if sh.step == 1 && !sh.in_progress {
                    sh.status = "Ready to download".into();
                }
            }
            Msg::InstallLabel(s) => self.install_label = s,
            Msg::InstallDone(r) => {
                sh.in_progress = false;
                sh.next_enabled = true;
                match r {
                    Ok(()) => {
                        sh.completed = true;
                        self.install_label = "Installation is complete!".into();
                        sh.status = "Installation complete!".into();
                    }
                    Err(e) => {
                        self.install_label = "Installation did not finish!".into();
                        sh.status = "Installation failed".into();
                        sh.dialogs.error_ui(&e);
                    }
                }
            }
        }
    }

    fn after_download_failure(&mut self, sh: &mut Shell) {
        self.cancel.store(true, Ordering::Relaxed);
        sh.in_progress = false;
        sh.next_enabled = true;
        if self.user == Some(UserType::Owner) {
            self.button = "Start Download".into();
            sh.status = "Ready to download".into();
        } else {
            self.button = "Retry".into();
            sh.status = "Patch failed. Try again.".into();
        }
    }
}

impl Flow for QuestInstall {
    fn background(&self) -> &'static str {
        "Echo2.jpg"
    }

    fn step_count(&self) -> usize {
        4
    }

    fn chip(&self, step: usize) -> &'static str {
        ["Type", "Download", "Install", "Done"][step]
    }

    fn substep_name(&self, step: usize, _sub: usize) -> String {
        ["Choose Type", "Download", "Install to Quest", "All Done"][step].into()
    }

    fn status_text(&self, _sh: &Shell, step: usize, _sub: usize) -> String {
        [
            "Choose your player type",
            "Ready to download",
            "Install to Quest",
            "Echo VR installation complete!",
        ][step]
            .into()
    }

    fn entered(&mut self, sh: &mut Shell) {
        match sh.step {
            0 => self.user = None, // forced to choose again
            1 => {
                self.apk_label = "Not started".into();
                self.data_label = "Not started".into();
                self.button = if self.user == Some(UserType::Owner) {
                    "Start Download".into()
                } else if self.patch.advanced {
                    "Start Patching".into()
                } else {
                    "Authorize with Discord".into()
                };
            }
            2 => {
                self.conn_started = false;
                self.install_label = "Not started yet".into();
            }
            _ => sh.next_enabled = true,
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
            2 => sh.completed,
            _ => true,
        }
    }

    fn poll(&mut self, sh: &mut Shell) {
        self.conn.poll(&mut sh.dialogs);
        for m in self.worker.drain() {
            self.apply_msg(sh, m);
        }
        if sh.dialogs.take("join-server").is_some_and(|a| a.is_yes()) {
            crate::core::platform::open_url(oauth::INVITE_URL);
        }
        if sh
            .dialogs
            .take("confirm-install")
            .is_some_and(|a| a.is_yes())
        {
            if let Some(ctx) = self.ctx.take() {
                self.start_install(sh, &ctx);
            }
        }
    }

    fn abort(&mut self, sh: &mut Shell) {
        self.cancel.store(true, Ordering::Relaxed);
        if sh.step == 1 {
            self.button = if self.user == Some(UserType::Owner) {
                "Start Download".into()
            } else {
                "Retry".into()
            };
            sh.status = "Ready to download".into();
            sh.next_enabled = true;
        }
    }

    fn content(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        let ctx = kit.ctx();
        let hx = ((cx - 450.0) / 2.0).floor();
        let bx = ((cx - Btn::Big.w()) / 2.0).floor();
        match sh.step {
            0 => {
                kit.header(
                    "Do you own Echo VR on your Meta account?",
                    hx,
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
                    sh.go(super::wizard::Nav::Advance);
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
                    sh.go(super::wizard::Nav::Advance);
                }
            }
            1 => {
                let owner = self.user == Some(UserType::Owner);
                let header = if owner {
                    "Download Echo VR client files"
                } else {
                    "Authorize and download patched Echo VR"
                };
                kit.header(header, hx, 5.0, 450.0, 46.0);
                let lx = ((cx - 440.0) / 2.0).floor();
                parts::progress_label(kit, &self.apk_label, 13.0, lx, 59.0, 440.0, 22.0);
                parts::progress_label(kit, &self.data_label, 13.0, lx, 89.0, 440.0, 22.0);
                if owner {
                    if kit.button(
                        "download",
                        Btn::Big,
                        &self.button,
                        18.0,
                        bx,
                        119.0,
                        true,
                        "Download the Echo VR APK and data files",
                    ) {
                        if sh.in_progress {
                            sh.go(super::wizard::Nav::Show(1, 0)); // routed through the abort confirm
                        } else {
                            self.start_owner_download(sh, &ctx);
                        }
                    }
                } else {
                    let mut label = std::mem::take(&mut self.button);
                    let ev = self.patch.show(
                        kit,
                        cx,
                        119.0,
                        &mut label,
                        "Authorize with Discord",
                        "Start Patching",
                        true,
                        |t| oauth::validate_apk_url(t).is_some(),
                        "Paste a direct URL to use a custom patch instead of generating one.",
                        (
                            "Authorize with Discord to get your patched APK",
                            "Download and install the patched APK from your pasted URL",
                        ),
                    );
                    self.button = label;
                    if ev.toggled {
                        self.patched_ready = false;
                    }
                    if ev.clicked {
                        if sh.in_progress {
                            sh.go(super::wizard::Nav::Show(1, 0));
                        } else {
                            self.start_patched_download(sh, &ctx);
                        }
                    }
                }
            }
            2 => {
                if !self.conn_started {
                    self.conn_started = true;
                    self.conn.check(&ctx, false);
                }
                kit.header("Install Echo VR to your Quest", hx, 5.0, 450.0, 50.0);
                self.conn.draw(kit, 60.0, cx);
                let mx = ((cx - Btn::Middle.w()) / 2.0).floor();
                let tip = "Check that your Quest is connected and has allowed this PC";
                if kit.button(
                    "connect",
                    Btn::Middle,
                    "Connect to Quest",
                    14.0,
                    mx,
                    88.0,
                    !self.conn.checking,
                    tip,
                ) {
                    self.conn.check(&ctx, true);
                }
                parts::progress_label(
                    kit,
                    &self.install_label,
                    15.0,
                    ((cx - 440.0) / 2.0).floor(),
                    128.0,
                    440.0,
                    28.0,
                );
                let tip = "Install the downloaded APK and data to your Quest";
                if kit.button(
                    "install",
                    Btn::Big,
                    "Install to Quest",
                    18.0,
                    bx,
                    166.0,
                    !sh.in_progress,
                    tip,
                ) {
                    // Installing uninstalls first, which wipes the app's local data.
                    self.ctx = Some(ctx.clone());
                    sh.dialogs.confirm(
                        "confirm-install",
                        "Install Echo VR",
                        "Installing replaces Echo VR on your Quest.\nThe installed app and its local data will be removed first.\n\nContinue?",
                        Icon::Warning,
                    );
                }
            }
            _ => parts::done_texts(
                kit,
                cx,
                55.0,
                "You're all set!",
                105.0,
                "Echo VR is ready to play on your Quest.",
            ),
        }
    }
}
