//! `FrameQuestUpdate`: Connect → Update → Done. The version gate runs when the Update
//! step opens, so the user never presses a button that can't work.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::dialogs::Icon;
use super::kit::{Btn, Kit};
use super::parts::{self, QuestConn, Worker};
use super::wizard::{Exit, Flow, Nav, Shell};
use crate::core::adb::devices::Status;
use crate::core::error::{HelpLink, UiError};
use crate::core::manifest::Manifest;
use crate::core::quest_update::{self, CheckStatus, VersionCheck};

enum Msg {
    Label(String),
    Checked(Box<CheckStatus>),
    Done(Result<(), Option<UiError>>),
}

pub struct QuestUpdate {
    conn: QuestConn,
    conn_started: bool,
    label: String,
    button: &'static str,
    update_enabled: bool,
    manifest: Option<Manifest>,
    cancel: Arc<AtomicBool>,
    worker: Worker<Msg>,
    check_started: bool,
}

impl Default for QuestUpdate {
    fn default() -> Self {
        QuestUpdate {
            conn: QuestConn::default(),
            conn_started: false,
            label: String::new(),
            button: "Start Update",
            update_enabled: false,
            manifest: None,
            cancel: Arc::new(AtomicBool::new(false)),
            worker: Worker::default(),
            check_started: false,
        }
    }
}

const MISMATCH_KEY: &str = "quest-mismatch";

impl QuestUpdate {
    fn on_checked(&mut self, sh: &mut Shell, st: CheckStatus) {
        if st.is_ok() {
            self.manifest = st.manifest;
            self.label = "Ready to update".into();
            self.update_enabled = true;
            return;
        }
        self.update_enabled = false;
        match st.result {
            VersionCheck::NoDevice => {
                self.label = "Quest disconnected".into();
                sh.dialogs
                    .error("No Quest detected", &st.detail, HelpLink::DeveloperMode);
            }
            VersionCheck::ManifestError => {
                self.label = "Could not check for updates".into();
                sh.dialogs
                    .error("Update check failed", &st.detail, HelpLink::None);
            }
            // NotInstalled and Mismatch mean the same to the user: reinstall.
            _ => {
                self.label = "Version mismatch — reinstall required".into();
                sh.dialogs.options(
                    MISMATCH_KEY,
                    "Echo VR version mismatch",
                    &format!(
                        "{}\n\nReinstall Echo VR on your Quest to continue.",
                        st.detail
                    ),
                    Icon::Warning,
                    &["Reinstall Echo VR", "Cancel"],
                );
            }
        }
    }

    fn start(&mut self, sh: &mut Shell, ctx: &egui::Context) {
        let Some(manifest) = self.manifest.clone() else {
            return;
        };
        sh.start_work();
        self.button = "Cancel";
        sh.next_enabled = false;
        sh.status = "Updating...".into();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        self.worker.spawn(ctx, move |tx| {
            let mut status = |s: String| tx.send(Msg::Label(s));
            let r = quest_update::apply(&manifest, &cancel, &mut status);
            tx.send(Msg::Done(r.map_err(|e| {
                if crate::core::http::is_cancelled(&e) {
                    None
                } else {
                    Some(UiError::from_anyhow(&e, "Update Failed"))
                }
            })));
        });
    }
}

impl Flow for QuestUpdate {
    fn background(&self) -> &'static str {
        "Echo2.jpg"
    }

    fn step_count(&self) -> usize {
        3
    }

    fn chip(&self, step: usize) -> &'static str {
        ["Connect", "Update", "Done"][step]
    }

    fn substep_name(&self, step: usize, _sub: usize) -> String {
        ["Connect to Quest", "Update", "All Done"][step].into()
    }

    fn status_text(&self, sh: &Shell, step: usize, _sub: usize) -> String {
        match step {
            0 => "Connect your Quest",
            1 if sh.in_progress => "Updating...",
            1 => "Ready to update",
            _ => "Echo VR Quest update complete!",
        }
        .into()
    }

    fn entered(&mut self, sh: &mut Shell) {
        match sh.step {
            0 => {
                // Auto-check on entry, silently -- the user hasn't asked for a popup yet.
                self.conn_started = false;
                sh.next_enabled = false;
            }
            1 => {
                self.label = "Checking your Echo VR version...".into();
                self.button = "Start Update";
                self.update_enabled = false;
                self.manifest = None;
                self.check_started = false;
                sh.next_enabled = false;
            }
            _ => {
                sh.completed = true;
                sh.next_enabled = true;
            }
        }
    }

    fn can_advance(&mut self, sh: &mut Shell) -> bool {
        match sh.step {
            0 => self.conn.status == Some(Status::Ready),
            1 => sh.completed,
            _ => true,
        }
    }

    fn poll(&mut self, sh: &mut Shell) {
        if let Some(st) = self.conn.poll(&mut sh.dialogs) {
            if sh.step == 0 {
                sh.next_enabled = st == Status::Ready;
            }
        }
        if let Some(a) = sh.dialogs.take(MISMATCH_KEY) {
            if a.is_yes() {
                sh.exit = Some(Exit::OpenQuestInstall);
            }
        }
        for m in self.worker.drain() {
            match m {
                Msg::Label(s) => self.label = s,
                Msg::Checked(st) => self.on_checked(sh, *st),
                Msg::Done(Ok(())) => {
                    self.label = "Update applied!".into();
                    sh.finish_work(true);
                    sh.next_enabled = true;
                    self.button = "Start Update";
                    sh.status = "Update applied!".into();
                    sh.go(Nav::Advance);
                }
                Msg::Done(Err(e)) => {
                    // Abort or cancel: back to a state the user can retry from.
                    sh.finish_work(false);
                    self.button = "Start Update";
                    self.update_enabled = true;
                    self.label = "Update did not finish".into();
                    sh.next_enabled = false;
                    sh.status = "Ready to update".into();
                    if let Some(e) = e {
                        sh.dialogs.error_ui(&e);
                    }
                }
            }
        }
    }

    fn abort(&mut self, _sh: &mut Shell) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn content(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        let ctx = kit.ctx();
        match sh.step {
            0 => {
                if !self.conn_started {
                    self.conn_started = true;
                    self.conn.check(&ctx, false);
                }
                kit.header(
                    "Connect your Quest",
                    ((cx - 450.0) / 2.0).floor(),
                    5.0,
                    450.0,
                    55.0,
                );
                self.conn.draw(kit, 68.0, cx);
                let bx = ((cx - Btn::Middle.w()) / 2.0).floor();
                let tip = "Check that your Quest is connected and has allowed this PC";
                if kit.button(
                    "connect",
                    Btn::Middle,
                    "Connect to Quest",
                    14.0,
                    bx,
                    106.0,
                    !self.conn.checking,
                    tip,
                ) {
                    sh.next_enabled = false;
                    self.conn.check(&ctx, true);
                }
            }
            1 => {
                if !self.check_started {
                    self.check_started = true;
                    self.worker.spawn(&ctx, |tx| {
                        let mut status = |s: String| tx.send(Msg::Label(s));
                        let st = quest_update::check_version(
                            quest_update::QUEST_MANIFEST_URL,
                            &mut status,
                        );
                        tx.send(Msg::Checked(Box::new(st)));
                    });
                }
                kit.header(
                    "Update Echo VR on your Quest",
                    ((cx - 450.0) / 2.0).floor(),
                    4.0,
                    450.0,
                    55.0,
                );
                let h = parts::label_height(kit, 14.0);
                parts::progress_label(
                    kit,
                    &self.label,
                    14.0,
                    ((cx - 440.0) / 2.0).floor(),
                    70.0,
                    440.0,
                    h,
                );
                let bx = ((cx - Btn::Middle.w()) / 2.0).floor();
                let enabled = self.update_enabled || sh.in_progress;
                if kit.button(
                    "update",
                    Btn::Middle,
                    self.button,
                    14.0,
                    bx,
                    110.0,
                    enabled,
                    "Copy the latest Echo VR update to your Quest",
                ) {
                    if sh.in_progress {
                        sh.dialogs.confirm(
                            "cancel-update",
                            "Install not done",
                            "Installation is still in progress.\n\nAbort and continue?",
                            Icon::Question,
                        );
                    } else {
                        self.start(sh, &ctx);
                    }
                }
                if sh.dialogs.take("cancel-update").is_some_and(|a| a.is_yes()) {
                    self.cancel.store(true, Ordering::Relaxed);
                    self.label = "Cancelling after the current file...".into();
                }
            }
            _ => parts::done_texts(
                kit,
                cx,
                55.0,
                "Update applied!",
                105.0,
                "Echo VR on your Quest has been updated.",
            ),
        }
    }
}
