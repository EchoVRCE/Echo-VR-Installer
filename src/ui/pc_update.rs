//! `FramePCUpdate`: Path → Update → Done.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::dialogs::Icon;
use super::kit::{Btn, Kit};
use super::parts::{self, PathField, Worker};
use super::wizard::{Flow, Nav, Shell};
use crate::core::{self, paths};

enum Msg {
    Status(String),
    Done(Result<(), (String, String)>),
}

pub struct PcUpdate {
    root: String,
    path: PathField,
    label: String,
    button: &'static str,
    cancel: Arc<AtomicBool>,
    worker: Worker<Msg>,
}

impl Default for PcUpdate {
    fn default() -> Self {
        let saved = crate::core::config::load_install_path().unwrap_or_else(|| "C:/EchoVR".into());
        let root = paths::normalize(&paths::resolve_install_root(&saved));
        PcUpdate {
            path: PathField { text: root.clone() },
            root,
            label: "0.00%".into(),
            button: "Start Update",
            cancel: Arc::new(AtomicBool::new(false)),
            worker: Worker::default(),
        }
    }
}

impl PcUpdate {
    fn commit(&mut self) {
        self.root = parts::commit_install_path(&self.path.text);
        self.path.text = self.root.clone();
    }

    fn detect(&mut self, sh: &mut Shell) {
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
                "Could not find a Meta/Oculus installation in the registry.\nIs the Meta Quest (Oculus) app installed?",
                Default::default(),
            );
            return;
        };
        let sep = if base.ends_with(['\\', '/']) {
            ""
        } else {
            "\\"
        };
        self.root = paths::normalize(&format!("{base}{sep}Software\\Software"));
        self.path.text = self.root.clone();
        crate::core::config::save_install_path(&self.root);
    }

    fn start(&mut self, sh: &mut Shell, ctx: &egui::Context) {
        sh.start_work();
        self.button = "Cancel";
        sh.next_enabled = false;
        sh.status = "Updating...".into();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let bin = paths::bin_path(&self.root);
        self.worker.spawn(ctx, move |tx| {
            let mut status = |s: String| tx.send(Msg::Status(s));
            let r = core::pc_update::apply(
                core::pc_update::PC_MANIFEST_URL,
                &bin,
                &cancel,
                &mut status,
            );
            tx.send(Msg::Done(r.map_err(|e| {
                if core::http::is_cancelled(&e) {
                    (String::new(), String::new())
                } else {
                    (core::pc_update::error_title(&e), format!("{e:#}"))
                }
            })));
        });
    }
}

impl Flow for PcUpdate {
    fn background(&self) -> &'static str {
        "EchoArena.jpg"
    }

    fn step_count(&self) -> usize {
        3
    }

    fn chip(&self, step: usize) -> &'static str {
        ["Path", "Update", "Done"][step]
    }

    fn substep_name(&self, step: usize, _sub: usize) -> String {
        ["Choose Path", "Update", "All Done"][step].into()
    }

    fn status_text(&self, sh: &Shell, step: usize, _sub: usize) -> String {
        match step {
            0 => "Choose your Echo VR install path",
            1 if sh.in_progress => "Updating...",
            1 => "Ready to update",
            _ => "Echo VR update complete!",
        }
        .into()
    }

    fn entered(&mut self, sh: &mut Shell) {
        if sh.step == 1 {
            self.label = "0.00%".into();
            self.button = "Start Update";
        }
        if sh.step == 2 {
            sh.completed = true;
            sh.next_enabled = true;
        }
    }

    fn can_advance(&mut self, _sh: &mut Shell) -> bool {
        true
    }

    fn poll(&mut self, sh: &mut Shell) {
        for m in self.worker.drain() {
            match m {
                Msg::Status(s) => self.label = s,
                Msg::Done(Ok(())) => {
                    self.label = "Update applied!".into();
                    sh.finish_work(true);
                    sh.next_enabled = true;
                    sh.status = "Update applied!".into();
                    self.button = "Start Update";
                    sh.go(Nav::Advance);
                }
                Msg::Done(Err((title, msg))) => {
                    sh.in_progress = false;
                    self.button = "Start Update";
                    sh.status = "Ready to update".into();
                    if title.is_empty() {
                        self.label = "0.00%".into();
                    } else {
                        self.label = "Update did not finish".into();
                        sh.dialogs.error(&title, &msg, Default::default());
                    }
                }
            }
        }
        if sh.step == 0 {
            // Only a real install can be updated.
            sh.next_enabled = paths::has_echo_install(&self.root);
        }
    }

    fn abort(&mut self, _sh: &mut Shell) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    fn content(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32) {
        let ctx = kit.ctx();
        match sh.step {
            0 => {
                kit.header(
                    "Choose your Echo VR install path",
                    ((cx - 450.0) / 2.0).floor(),
                    5.0,
                    450.0,
                    55.0,
                );
                let fx = ((cx - 440.0) / 2.0).floor();
                let valid = paths::has_echo_install(&self.root);
                if self
                    .path
                    .show(
                        kit,
                        "path",
                        fx,
                        70.0,
                        valid,
                        (fx + 445.0, 64.0, 90.0, 34.0),
                        "",
                    )
                    .commit
                {
                    self.commit();
                }
                let bx = ((cx - Btn::Small.w()) / 2.0).floor();
                if kit.button(
                    "choose",
                    Btn::Small,
                    "Choose path",
                    11.0,
                    bx,
                    102.0,
                    true,
                    "Select the folder where Echo VR is installed",
                ) {
                    if let Some(p) = parts::choose_folder() {
                        self.path.text = p;
                        self.commit();
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
                    "Auto-detect Echo VR from Oculus installation",
                ) {
                    self.detect(sh);
                }
            }
            1 => {
                kit.header(
                    "Downloading Echo VR update",
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
                if kit.button(
                    "update",
                    Btn::Middle,
                    self.button,
                    14.0,
                    bx,
                    110.0,
                    true,
                    "Download the latest Echo VR game update",
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
                }
            }
            _ => {
                parts::done_texts(
                    kit,
                    cx,
                    20.0,
                    "Update applied!",
                    70.0,
                    "Echo VR has been updated.",
                );
                let bx = ((cx - Btn::Big.w()) / 2.0).floor();
                if kit.button(
                    "open",
                    Btn::Big,
                    "Open Install Folder",
                    18.0,
                    bx,
                    115.0,
                    true,
                    "Open the Echo VR bin/win10 folder",
                ) {
                    if self.root.is_empty() {
                        sh.dialogs.error(
                            "No Install Path",
                            "Echo VR is not installed. Please download and install Echo VR first.",
                            Default::default(),
                        );
                    } else if let Err(e) = core::platform::open_folder(&paths::bin_path(&self.root))
                    {
                        sh.dialogs.error(
                            "Couldn't open folder",
                            &format!("{e:#}"),
                            Default::default(),
                        );
                    }
                }
            }
        }
    }
}
