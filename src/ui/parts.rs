//! Pieces shared by several wizards: a worker channel, the editable install-path field,
//! the "patch options" panel, and the Quest connection row.

use std::sync::mpsc::{channel, Receiver, Sender};

use egui::Color32;

use super::dialogs::{Answer, DialogHost};
use super::kit::{Btn, Kit};
use super::theme;
use crate::core::adb::{self, devices::Device, devices::Status};
use crate::core::error;
use crate::core::paths;

// ---- worker messages ----

/// Background threads report back through this; every send wakes the UI.
pub struct Worker<M> {
    tx: Sender<M>,
    rx: Receiver<M>,
}

pub struct Tx<M> {
    tx: Sender<M>,
    ctx: egui::Context,
}

impl<M> Clone for Tx<M> {
    fn clone(&self) -> Self {
        Tx {
            tx: self.tx.clone(),
            ctx: self.ctx.clone(),
        }
    }
}

impl<M: Send + 'static> Tx<M> {
    pub fn send(&self, m: M) {
        let _ = self.tx.send(m);
        self.ctx.request_repaint();
    }
}

impl<M: Send + 'static> Default for Worker<M> {
    fn default() -> Self {
        let (tx, rx) = channel();
        Worker { tx, rx }
    }
}

impl<M: Send + 'static> Worker<M> {
    pub fn tx(&self, ctx: &egui::Context) -> Tx<M> {
        Tx {
            tx: self.tx.clone(),
            ctx: ctx.clone(),
        }
    }

    pub fn drain(&self) -> Vec<M> {
        self.rx.try_iter().collect()
    }

    /// Runs `f` on a new thread with a sender.
    pub fn spawn(&self, ctx: &egui::Context, f: impl FnOnce(Tx<M>) + Send + 'static) {
        let tx = self.tx(ctx);
        std::thread::spawn(move || f(tx));
    }
}

// ---- install path field ----

/// The 440-wide editable, pastable install-path field with its ✓/✗ indicator.
#[derive(Default)]
pub struct PathField {
    pub text: String,
}

pub struct PathEvent {
    /// Enter, focus loss, or the indicator's clear -- time to resolve/save.
    pub commit: bool,
}

impl PathField {
    /// `ind` = (x, y, w, h) of the indicator label; the mark is left-aligned, v-centered.
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        kit: &mut Kit,
        key: &str,
        x: f32,
        y: f32,
        valid: bool,
        ind: (f32, f32, f32, f32),
        tip: &str,
    ) -> PathEvent {
        let r = kit.text_field(
            key,
            &mut self.text,
            x,
            y,
            440.0,
            24.0,
            12.0,
            "",
            theme::FIELD_BG,
            tip,
        );
        let (ix, iy, iw, ih) = ind;
        let size = if valid { 26.0 } else { 22.0 };
        let color = if valid {
            theme::MARK_OK
        } else {
            theme::MARK_BAD
        };
        kit.mark(valid, color, size, ix, iy + ((ih - size) / 2.0).floor());
        let clear = kit
            .hand_area(
                &format!("{key}-ind"),
                ix,
                iy,
                iw.min(size + 4.0),
                ih,
                "Click the check / cross icon to clear this field",
            )
            .clicked;
        if clear {
            self.text.clear();
        }
        PathEvent {
            commit: r.committed || clear,
        }
    }
}

/// Opens a folder picker; `None` when cancelled.
pub fn choose_folder() -> Option<String> {
    rfd::FileDialog::new()
        .pick_folder()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Resolves what the user typed to an install root and saves it (`commitPathField`).
pub fn commit_install_path(input: &str) -> String {
    let root = paths::normalize(&paths::resolve_install_root(input));
    crate::core::config::save_install_path(&root);
    root
}

/// The saved path, or the platform default, resolved to an install root.
pub fn initial_install_root() -> String {
    let p = crate::core::config::load_install_path().unwrap_or_else(paths::default_install_root);
    paths::normalize(&p)
}

// ---- patch options panel ----

/// "Authorize with Discord" + "Advanced Options" checkbox + custom URL row, in one box.
#[derive(Default)]
pub struct PatchOptions {
    pub advanced: bool,
    pub url: String,
}

pub struct PatchEvent {
    pub clicked: bool,
    /// The checkbox flipped -- callers drop staged-download reuse.
    pub toggled: bool,
}

impl PatchOptions {
    /// `label` is the button's current text (it cycles through Please Wait/Retry/Done);
    /// toggling resets it to `default_label` / `custom_label`.
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        kit: &mut Kit,
        cx: f32,
        top: f32,
        label: &mut String,
        default_label: &str,
        custom_label: &str,
        enabled: bool,
        valid: fn(&str) -> bool,
        field_tip: &str,
        button_tip: (&str, &str),
    ) -> PatchEvent {
        let (pw, pad, gap, cb_h, row_h) = (440.0, 10.0, 6.0, 18.0, 24.0);
        let px = ((cx - pw) / 2.0).floor();
        let btn_h = 50.0;
        let h = if self.advanced {
            pad + btn_h + gap + cb_h + gap + row_h + pad
        } else {
            pad + btn_h + gap + cb_h + pad
        };
        kit.section_box(px, top, pw, h, 15.0, theme::SECTION_FILL);

        let tip = if self.advanced {
            button_tip.1
        } else {
            button_tip.0
        };
        let clicked = kit.button(
            "patch-btn",
            Btn::Big,
            label,
            18.0,
            ((cx - Btn::Big.w()) / 2.0).floor(),
            top + pad,
            enabled,
            tip,
        );

        let mut adv = self.advanced;
        let toggled = kit.checkbox(
            "patch-adv",
            &mut adv,
            "Advanced Options",
            12.0,
            px,
            top + pad + btn_h + gap,
            pw,
            cb_h,
            true,
            enabled,
            "",
        );
        if toggled {
            self.advanced = adv;
            *label = if adv { custom_label } else { default_label }.to_string();
            if adv {
                kit.request_focus("patch-url");
            }
        }

        if self.advanced {
            let row_y = top + pad + btn_h + gap + cb_h + gap;
            let (icon_w, ind_w) = (26.0, 24.0);
            let field_x = px + pad;
            let icon_x = px + pw - pad - icon_w;
            let ind_x = icon_x - 6.0 - ind_w;
            let field_w = ind_x - 8.0 - field_x;
            let t = self.url.trim().to_string();
            let bg = if t.is_empty() {
                theme::FIELD_BG
            } else if valid(&t) {
                theme::FIELD_BG_VALID
            } else {
                theme::FIELD_BG_INVALID
            };
            kit.text_field(
                "patch-url",
                &mut self.url,
                field_x,
                row_y,
                field_w,
                row_h,
                12.0,
                "Paste patch URL here…",
                bg,
                field_tip,
            );
            if !t.is_empty() {
                let ok = valid(&t);
                kit.mark(
                    ok,
                    if ok { theme::MARK_OK } else { theme::MARK_BAD },
                    22.0,
                    ind_x,
                    row_y - 2.0 + 3.0,
                );
                if kit
                    .hand_area(
                        "patch-ind",
                        ind_x,
                        row_y - 2.0,
                        ind_w,
                        28.0,
                        "Click the check / cross icon to clear this field",
                    )
                    .clicked
                {
                    self.url.clear();
                }
            }
            kit.clipboard_icon(
                icon_x + 3.0,
                row_y + 2.0,
                20.0,
                Color32::from_rgb(230, 230, 230),
            );
            if kit
                .hand_area(
                    "patch-paste",
                    icon_x,
                    row_y,
                    icon_w,
                    row_h,
                    "Paste a link from your clipboard",
                )
                .clicked
            {
                if let Some(clip) = arboard::Clipboard::new()
                    .ok()
                    .and_then(|mut c| c.get_text().ok())
                {
                    if !clip.trim().is_empty() {
                        self.url = clip.trim().to_string();
                    }
                }
            }
        }
        PatchEvent { clicked, toggled }
    }
}

// ---- Quest connection row ----

enum ConnMsg {
    Done(Status, bool),
}

/// "Checking Quest connection..." → ✓ Quest connected (model) / ✗ reason.
#[derive(Default)]
pub struct QuestConn {
    worker: Worker<ConnMsg>,
    pub checking: bool,
    pub status: Option<Status>,
    picker: Option<Vec<Device>>,
}

const PICKER_KEY: &str = "quest-picker";

impl QuestConn {
    pub fn check(&mut self, ctx: &egui::Context, interactive: bool) {
        self.checking = true;
        self.status = None;
        self.worker.spawn(ctx, move |tx| {
            let st = adb::bundle::binary()
                .map(|_| adb::connection_status())
                .unwrap_or(Status::None);
            tx.send(ConnMsg::Done(st, interactive));
        });
    }

    /// Returns the final status once a check (and a possible device pick) completes.
    pub fn poll(&mut self, dialogs: &mut DialogHost) -> Option<Status> {
        let mut result = None;
        for ConnMsg::Done(st, interactive) in self.worker.drain() {
            self.checking = false;
            self.status = Some(st);
            if !interactive {
                result = Some(st);
                continue;
            }
            match st {
                Status::Ambiguous => {
                    // Several usable devices and nothing tells them apart: ask, don't guess.
                    let devices = adb::last_selection().pickable();
                    dialogs.device_picker(PICKER_KEY, devices.clone());
                    self.picker = Some(devices);
                }
                Status::Unauthorized => dialogs.error_ui(&error::quest_unauthorized()),
                Status::None => dialogs.error_ui(&error::quest_not_found()),
                Status::Ready => {}
            }
            if st != Status::Ambiguous {
                result = Some(st);
            }
        }
        if let Some(a) = dialogs.take(PICKER_KEY) {
            let devices = self.picker.take().unwrap_or_default();
            match a {
                Answer::Button(i) if i < devices.len() => {
                    adb::set_preferred(&devices[i].serial);
                    self.status = Some(Status::Ready);
                    result = Some(Status::Ready);
                }
                _ => result = self.status,
            }
        }
        result
    }

    /// The status row at `y`, full content width.
    pub fn draw(&self, kit: &mut Kit, y: f32, cx: f32) {
        let (text, mark) = match (self.checking, self.status) {
            (true, _) | (_, None) => ("Checking Quest connection...".to_string(), None),
            (_, Some(Status::Ready)) => {
                let model =
                    adb::target_device().and_then(|d| d.model().map(|m| m.replace('_', " ")));
                let t = match model {
                    Some(m) if !m.is_empty() => format!("Quest connected ({m})"),
                    _ => "Quest connected".into(),
                };
                (t, Some((true, theme::QUEST_OK)))
            }
            (_, Some(Status::Ambiguous)) => (
                "Several devices connected".into(),
                Some((false, theme::MARK_BAD)),
            ),
            (_, Some(Status::Unauthorized)) => (
                "Quest found — not authorized".into(),
                Some((false, theme::MARK_BAD)),
            ),
            (_, Some(Status::None)) => ("No Quest detected".into(), Some((false, theme::MARK_BAD))),
        };
        let color = if mark.is_some() {
            theme::WHITE
        } else {
            theme::LIGHT_GRAY
        };
        let font = theme::arial_bold(14.0);
        let tw = kit.text_size(&text, font.clone()).x;
        let icon_w = if mark.is_some() { 18.0 + 6.0 } else { 0.0 };
        let x = ((cx - tw - icon_w) / 2.0).floor();
        if let Some((ok, c)) = mark {
            kit.mark(ok, c, 18.0, x, y + 3.0);
        }
        kit.text_left(x + icon_w, y, 24.0, &text, font, color);
    }
}

/// `SpecialLabel` used for progress readouts: white-ish box, black Conthrax text.
pub fn progress_label(kit: &Kit, text: &str, size: f32, x: f32, y: f32, w: f32, h: f32) {
    kit.special_label(x, y, w, h, text, size, theme::PROGRESS_BG, theme::BLACK);
}

/// Natural height of a `SpecialLabel` in Conthrax `size`.
pub fn label_height(kit: &Kit, size: f32) -> f32 {
    kit.special_label_size("0", size).y
}

/// The "You're all set!" / "Update applied!" done text pair.
pub fn done_texts(kit: &Kit, cx: f32, y1: f32, title: &str, y2: f32, subtitle: &str) {
    kit.text_center(
        0.0,
        y1,
        cx,
        40.0,
        title,
        theme::arial_bold(24.0),
        theme::DONE_GREEN,
        None,
    );
    kit.text_center(
        0.0,
        y2,
        cx,
        24.0,
        subtitle,
        theme::arial(16.0),
        theme::WHITE,
        None,
    );
}
