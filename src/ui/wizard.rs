//! `BaseWizard`: the shared shell of every install/update flow -- status bar with the
//! pulse animation, sidebar with sub-steps, the chip navigation bar with Back/Next, the
//! big section box and the TipBox (see `frame.rs`). Each concrete wizard implements
//! [`Flow`]. Wizards run inside the launcher window.

use std::time::Duration;

use super::assets::Assets;
use super::dialogs::{DialogHost, Icon};
use super::frame::{self, Chip, Mark, SideRow, CONTENT_W, CONTENT_X};
use super::kit::{Btn, Kit};
use super::theme;
use super::tipbox::{self, TipBox};

const CONTENT_Y: f32 = 72.0;
const CONTENT_H: f32 = 245.0;
const ABORT_KEY: &str = "wizard-abort";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Show(usize, usize),
    Advance,
    Back,
    Chip(usize),
    Close,
    /// The app window is closing.
    Quit,
}

/// What a closed wizard asks the app to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Closed,
    OpenQuestInstall,
    /// Close the app (its window was closed while work ran, and the user agreed).
    Quit,
}

pub struct Shell {
    pub fw: f32,
    pub step: usize,
    pub sub: usize,
    pub in_progress: bool,
    pub completed: bool,
    pub status: String,
    pub back_enabled: bool,
    pub next_enabled: bool,
    pub tipbox: TipBox,
    pub dialogs: DialogHost,
    pub exit: Option<Exit>,
    nav: Option<Nav>,
    pending: Option<Nav>,
    pulse: frame::Pulse,
}

impl Shell {
    fn new(fw: f32) -> Self {
        Shell {
            fw,
            step: 0,
            sub: 0,
            in_progress: false,
            completed: false,
            status: String::new(),
            back_enabled: false,
            next_enabled: false,
            tipbox: TipBox::default(),
            dialogs: DialogHost::default(),
            exit: None,
            nav: None,
            pending: None,
            pulse: frame::Pulse::default(),
        }
    }

    /// Queue a navigation, confirming first when work is in progress (`confirmAbortDownload`).
    pub fn go(&mut self, nav: Nav) {
        self.nav = Some(nav);
    }

    pub fn content_w(&self) -> f32 {
        self.fw - CONTENT_X - 10.0 - 20.0
    }

    /// Marks work as started: pulse on.
    pub fn start_work(&mut self) {
        self.in_progress = true;
        self.completed = false;
    }

    pub fn finish_work(&mut self, ok: bool) {
        self.in_progress = false;
        self.completed = ok;
    }

    /// `resetAfterError`
    pub fn reset_after_error(&mut self) {
        self.in_progress = false;
        self.status = "Patch failed. Try again.".into();
    }
}

pub trait Flow {
    fn background(&self) -> &'static str;
    fn step_count(&self) -> usize;
    fn chip(&self, step: usize) -> &'static str;
    fn substep_count(&self, step: usize) -> usize {
        let _ = step;
        1
    }
    fn substep_name(&self, step: usize, sub: usize) -> String;
    fn status_text(&self, sh: &Shell, step: usize, sub: usize) -> String;
    /// Called before a step is shown; may redirect. `from` is the step being left.
    fn enter(&mut self, sh: &mut Shell, step: usize, sub: usize, from: usize) -> (usize, usize) {
        let _ = (sh, from);
        (step, sub)
    }
    /// After the shell has reset its state for the step.
    fn entered(&mut self, sh: &mut Shell) {
        let _ = sh;
    }
    fn content(&mut self, sh: &mut Shell, kit: &mut Kit, cx: f32);
    fn can_advance(&mut self, sh: &mut Shell) -> bool {
        let _ = sh;
        true
    }
    /// Return true when the chip click was handled.
    fn chip_click(&mut self, sh: &mut Shell, step: usize) -> bool {
        let _ = (sh, step);
        false
    }
    /// Drain worker messages and dialog answers.
    fn poll(&mut self, sh: &mut Shell) {
        let _ = sh;
    }
    /// The user confirmed aborting the running work.
    fn abort(&mut self, sh: &mut Shell) {
        let _ = sh;
    }
}

pub struct Wizard<F: Flow> {
    pub flow: F,
    pub sh: Shell,
    id: &'static str,
}

impl<F: Flow> Wizard<F> {
    pub fn new(id: &'static str, flow: F) -> Self {
        let mut wz = Wizard {
            flow,
            sh: Shell::new(frame::W),
            id,
        };
        wz.show_step(0, 0);
        wz
    }

    fn show_step(&mut self, s: usize, sub: usize) {
        let from = self.sh.step;
        let (s, sub) = self.flow.enter(&mut self.sh, s, sub, from);
        let sh = &mut self.sh;
        sh.step = s;
        sh.sub = sub;
        sh.completed = false;
        sh.in_progress = false;
        sh.back_enabled = !(s == 0 && sub == 0);
        sh.next_enabled = s + 1 < self.flow.step_count();
        sh.status = self.flow.status_text(sh, s, sub);
        self.flow.entered(&mut self.sh);
    }

    fn is_last(&self) -> bool {
        self.sh.step + 1 >= self.flow.step_count()
            && self.sh.sub + 1 >= self.flow.substep_count(self.sh.step)
    }

    fn perform(&mut self, nav: Nav) {
        match nav {
            Nav::Show(s, sub) => self.show_step(s, sub),
            Nav::Advance => {
                // "Finish" closes on the first click (the Java wizard needed two).
                if self.is_last() {
                    self.sh.exit.get_or_insert(Exit::Closed);
                    return;
                }
                if !self.flow.can_advance(&mut self.sh) {
                    return;
                }
                let sc = self.flow.substep_count(self.sh.step);
                if self.sh.sub + 1 < sc {
                    self.show_step(self.sh.step, self.sh.sub + 1);
                } else {
                    self.show_step(self.sh.step + 1, 0);
                }
            }
            Nav::Back => {
                if self.sh.sub > 0 {
                    self.show_step(self.sh.step, self.sh.sub - 1);
                } else if self.sh.step > 0 {
                    let p = self.sh.step - 1;
                    self.show_step(p, self.flow.substep_count(p) - 1);
                }
            }
            Nav::Chip(i) => {
                if self.flow.chip_click(&mut self.sh, i) {
                    return;
                }
                if i < self.sh.step {
                    self.show_step(i, self.flow.substep_count(i) - 1);
                } else {
                    self.show_step(i, 0);
                }
            }
            Nav::Close => {
                self.sh.exit.get_or_insert(Exit::Closed);
            }
            Nav::Quit => {
                self.sh.exit.get_or_insert(Exit::Quit);
            }
        }
    }

    fn process_nav(&mut self) {
        if let Some(a) = self.sh.dialogs.take(ABORT_KEY) {
            if let (true, Some(nav)) = (a.is_yes(), self.sh.pending.take()) {
                self.flow.abort(&mut self.sh);
                self.sh.in_progress = false;
                self.perform(nav);
            }
            self.sh.pending = None;
        }
        while let Some(nav) = self.sh.nav.take() {
            if self.sh.in_progress {
                self.sh.pending = Some(nav);
                self.sh.dialogs.confirm(
                    ABORT_KEY,
                    "Install not done",
                    "Installation is still in progress.\n\nAbort and continue?",
                    Icon::Question,
                );
                break;
            }
            self.perform(nav);
        }
    }

    /// One frame of the wizard inside the launcher window. Returns `Some` once it closed.
    pub fn frame(&mut self, ui: &mut egui::Ui, assets: &Assets) -> Option<Exit> {
        self.flow.poll(&mut self.sh);
        self.process_nav();
        let blocked = self.sh.dialogs.is_open();
        let own_rect = ui.input(|i| i.viewport().outer_rect);
        // Esc goes back to the launcher (asking first while work runs), unless it just
        // leaves a text field.
        let esc = !blocked
            && ui.memory(|m| m.focused().is_none())
            && ui.input(|i| i.key_pressed(egui::Key::Escape));
        let mut kit = Kit::new(ui, assets, self.id, blocked);
        self.draw(&mut kit);
        let ctx = kit.ctx();
        if esc {
            self.sh.go(Nav::Close);
        }
        self.sh.dialogs.show(&ctx, assets, own_rect);
        if self.sh.in_progress {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        // Navigation and dialog answers are handled at the start of the next frame.
        if self.sh.nav.is_some() || self.sh.dialogs.has_answers() || self.sh.exit.is_some() {
            ctx.request_repaint();
        }
        self.sh.exit
    }

    fn draw(&mut self, kit: &mut Kit) {
        let pulse = self.sh.pulse.tick(self.sh.in_progress);
        let tip_x = CONTENT_X + ((CONTENT_W - tipbox::W) / 2.0).floor();
        let tip_y = CONTENT_Y + CONTENT_H + 10.0 + 8.0;

        frame::background(kit, self.flow.background());
        self.sh.tipbox.draw_clippy(kit, tip_x, tip_y);

        self.draw_bar(kit, pulse);
        frame::boxes(kit);

        // Content.
        let cx = self.sh.content_w();
        kit.at(CONTENT_X + 10.0, CONTENT_Y, |k| {
            self.flow.content(&mut self.sh, k, cx)
        });

        // Status bar, drawn above the content like the Swing z-order.
        let fill = if self.sh.in_progress {
            frame::pulse_fill(pulse)
        } else if self.sh.completed {
            theme::STATUS_DONE
        } else {
            theme::STATUS_IDLE
        };
        frame::status_bar(kit, &self.sh.status, fill);

        self.draw_sidebar(kit);

        // The TipBox last among tip-bearing widgets, so it sees this frame's hover.
        self.sh.tipbox.draw(kit, tip_x, tip_y);
    }

    fn draw_bar(&mut self, kit: &mut Kit, pulse: f32) {
        let n = self.flow.step_count();
        let labels: Vec<&str> = (0..n).map(|i| self.flow.chip(i)).collect();
        let states: Vec<Chip> = (0..n)
            .map(|i| {
                if i < self.sh.step {
                    Chip::Done
                } else if i == self.sh.step {
                    if self.sh.in_progress {
                        Chip::Busy(pulse)
                    } else {
                        Chip::Current
                    }
                } else {
                    Chip::Upcoming
                }
            })
            .collect();
        let (_, total) = frame::chip_widths(kit, &labels);

        frame::bottom_bar(kit);
        let chips_x = CONTENT_X + ((CONTENT_W - total) / 2.0).floor();
        if let Some(i) = frame::chips(kit, "chip", chips_x, &labels, &states, true, &[]) {
            self.sh.go(Nav::Chip(i));
        }

        let btn_y = frame::bar_button_y();
        let chip_right = chips_x + total;
        let left_gap = chips_x - CONTENT_X;
        let right_gap = CONTENT_X + CONTENT_W - chip_right;
        let bw = Btn::Small.w();
        let back_x = (CONTENT_X + ((left_gap - bw) / 2.0).floor()).max(CONTENT_X);
        let next_x =
            (chip_right + ((right_gap - bw) / 2.0).floor()).min(CONTENT_X + CONTENT_W - bw);
        if kit.button(
            "back",
            Btn::Small,
            "< Back",
            11.0,
            back_x,
            btn_y,
            self.sh.back_enabled,
            "",
        ) {
            self.sh.go(Nav::Back);
        }
        let last = self.sh.step + 1 >= n;
        let next_text = if last { "Finish" } else { "Next >" };
        if kit.button(
            "next",
            Btn::Small,
            next_text,
            11.0,
            next_x,
            btn_y,
            self.sh.next_enabled,
            "",
        ) {
            self.sh.go(Nav::Advance);
        }
    }

    fn draw_sidebar(&mut self, kit: &mut Kit) {
        let sc = self.flow.substep_count(self.sh.step);
        let names: Vec<String> = (0..sc)
            .map(|i| self.flow.substep_name(self.sh.step, i))
            .collect();
        let rows: Vec<SideRow> = names
            .iter()
            .enumerate()
            .map(|(i, name)| SideRow {
                label: name,
                mark: if i < self.sh.sub {
                    Mark::Done
                } else if i == self.sh.sub {
                    Mark::Current
                } else {
                    Mark::Open
                },
                clickable: i < self.sh.sub,
                tip: "",
            })
            .collect();
        let heading = format!("Step {}", self.sh.step + 1);
        let (clicked, _) = frame::side_list(kit, "side", frame::SECTION_Y + 10.0, &heading, &rows);
        if let Some(i) = clicked {
            let step = self.sh.step;
            self.sh.go(Nav::Show(step, i));
        }
        if frame::side_link(kit, "to-launcher", "< Launcher", "Back to the launcher") {
            self.sh.go(Nav::Close);
        }
    }
}

/// Type-erased wizard for the app to hold.
pub trait WizardWindow {
    /// One frame inside the launcher window; `Some` once the wizard closed.
    fn frame(&mut self, ui: &mut egui::Ui, assets: &Assets) -> Option<Exit>;
    /// The app window is closing: true to keep it open while work runs (the wizard
    /// asks to abort first, then exits with [`Exit::Quit`]).
    fn hold_close(&mut self) -> bool;
    /// Snapshot harness: jump straight to a step.
    fn debug_goto(&mut self, step: usize, sub: usize);
}

impl<F: Flow> WizardWindow for Wizard<F> {
    fn hold_close(&mut self) -> bool {
        if !self.sh.in_progress {
            return false;
        }
        if !self.sh.dialogs.is_open() {
            self.sh.go(Nav::Quit);
        }
        true
    }

    fn debug_goto(&mut self, step: usize, sub: usize) {
        self.show_step(step, sub);
    }

    fn frame(&mut self, ui: &mut egui::Ui, assets: &Assets) -> Option<Exit> {
        Wizard::frame(self, ui, assets)
    }
}
