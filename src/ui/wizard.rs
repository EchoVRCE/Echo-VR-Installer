//! `BaseWizard`: the shared shell of every install/update window -- status bar with the
//! pulse animation, sidebar with sub-steps, the chip navigation bar with Back/Next, the
//! big section box and the TipBox. Each concrete wizard implements [`Flow`].

use std::time::{Duration, Instant};

use egui::{Color32, ViewportBuilder, ViewportCommand, ViewportId};

use super::assets::{self, Assets};
use super::dialogs::{DialogHost, Icon};
use super::kit::{Btn, Kit};
use super::theme::{self};
use super::tipbox::{self, TipBox};

pub const FH: f32 = 594.0;
const SIDEBAR_W: f32 = 120.0;
const CONTENT_X: f32 = SIDEBAR_W + 30.0; // 150
const CONTENT_Y: f32 = 72.0;
const CONTENT_H: f32 = 245.0;
const BAR_Y: f32 = FH - 74.0; // 520
const BAR_H: f32 = 42.0;
const ABORT_KEY: &str = "wizard-abort";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Show(usize, usize),
    Advance,
    Back,
    Chip(usize),
    Close,
}

/// What a closed wizard asks the app to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Closed,
    OpenQuestInstall,
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
    phase: f32,
    last_tick: Instant,
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
            phase: 0.0,
            last_tick: Instant::now(),
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
        let (w, h) = assets::native_size(flow.background());
        let fw = (w as f32 * FH / h as f32).floor();
        let mut wz = Wizard {
            flow,
            sh: Shell::new(fw),
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

    /// Shows the wizard window. Returns `Some` once it closed.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        assets: &Assets,
        parent: Option<egui::Rect>,
    ) -> Option<Exit> {
        let fw = self.sh.fw;
        let mut builder = ViewportBuilder::default()
            .with_title(crate::version::VERSION_TITLE)
            .with_inner_size([fw, FH])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_icon(std::sync::Arc::new(assets::icon()));
        if let Some(p) = parent {
            builder =
                builder.with_position(egui::pos2(p.center().x - fw / 2.0, p.center().y - FH / 2.0));
        }
        let vid = ViewportId::from_hash_of(("wizard", self.id));
        ctx.show_viewport_immediate(vid, builder, |ui, _| {
            super::snapshot::capture(ui);
            if ui.input(|i| i.viewport().close_requested()) {
                ui.ctx().send_viewport_cmd(ViewportCommand::CancelClose);
                if !self.sh.dialogs.is_open() {
                    self.sh.go(Nav::Close);
                }
            }
            self.frame(ui, assets);
        });
        self.sh.exit
    }

    /// One frame of the wizard inside `ui` (its own window, or the root for snapshots).
    pub fn frame(&mut self, ui: &mut egui::Ui, assets: &Assets) {
        self.flow.poll(&mut self.sh);
        self.process_nav();
        let blocked = self.sh.dialogs.is_open();
        let own_rect = ui.input(|i| i.viewport().outer_rect);
        let mut kit = Kit::new(ui, assets, self.id, blocked);
        self.draw(&mut kit);
        let ctx = kit.ctx();
        self.sh.dialogs.show(&ctx, assets, own_rect);
        if self.sh.in_progress {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        // Navigation and dialog answers are handled at the start of the next frame.
        if self.sh.nav.is_some() || self.sh.dialogs.has_answers() || self.sh.exit.is_some() {
            ctx.request_repaint();
        }
    }

    fn draw(&mut self, kit: &mut Kit) {
        // Pulse: phase += 0.15 every 50 ms while working.
        let now = Instant::now();
        if self.sh.in_progress {
            let ticks = (now - self.sh.last_tick).as_millis() as f32 / 50.0;
            self.sh.phase += 0.15 * ticks;
        }
        self.sh.last_tick = now;
        let pulse = self.sh.phase.sin() * 0.5 + 0.5;

        let fw = self.sh.fw;
        let cbw = fw - CONTENT_X - 10.0;
        let tip_x = CONTENT_X + ((cbw - tipbox::W) / 2.0).floor();
        let tip_y = CONTENT_Y + CONTENT_H + 10.0 + 8.0;
        let section_y = CONTENT_Y - 20.0;
        let section_h = tip_y + tipbox::H - section_y + 20.0;

        kit.image(self.flow.background(), 0.0, 0.0, fw, FH);
        self.sh.tipbox.draw_clippy(kit, tip_x, tip_y);

        self.draw_bar(kit, cbw, pulse);

        // Sidebar box + big section box.
        kit.section_box(
            10.0,
            section_y,
            SIDEBAR_W + 10.0,
            section_h,
            15.0,
            theme::SIDEBAR_FILL,
        );
        kit.section_box(
            CONTENT_X,
            section_y,
            cbw,
            section_h,
            15.0,
            theme::SECTION_FILL,
        );

        // Content.
        let cx = self.sh.content_w();
        kit.at(CONTENT_X + 10.0, CONTENT_Y, |k| {
            self.flow.content(&mut self.sh, k, cx)
        });

        // Status bar, drawn above the content like the Swing z-order.
        let fill = if self.sh.in_progress {
            Color32::from_rgb(
                (50.0 + pulse * 40.0) as u8,
                (90.0 + pulse * 50.0) as u8,
                (150.0 + pulse * 60.0) as u8,
            )
        } else if self.sh.completed {
            theme::STATUS_DONE
        } else {
            theme::STATUS_IDLE
        };
        kit.round_box(
            CONTENT_X,
            10.0,
            cbw,
            32.0,
            8.0,
            fill,
            Some(theme::BOX_BORDER),
        );
        kit.text_center(
            CONTENT_X,
            10.0,
            cbw,
            32.0,
            &self.sh.status,
            theme::arial_bold(14.0),
            theme::WHITE,
            None,
        );

        self.draw_sidebar(kit, section_y + 10.0);

        // The TipBox last among tip-bearing widgets, so it sees this frame's hover.
        self.sh.tipbox.draw(kit, tip_x, tip_y);
    }

    fn draw_bar(&mut self, kit: &mut Kit, cbw: f32, pulse: f32) {
        let n = self.flow.step_count();
        let chip_font = theme::conthrax(9.0);
        let widths: Vec<f32> = (0..n)
            .map(|i| {
                (kit.text_size(self.flow.chip(i), chip_font.clone())
                    .x
                    .round()
                    + 16.0)
                    .clamp(40.0, 74.0)
            })
            .collect();
        let gap = 12.0;
        let total = widths.iter().sum::<f32>() + gap * (n - 1) as f32;

        kit.section_box(CONTENT_X, BAR_Y, cbw, BAR_H, 15.0, theme::SIDEBAR_FILL);

        let chips_x = CONTENT_X + ((cbw - total) / 2.0).floor();
        let chip_h = 24.0;
        let chip_y = BAR_Y + ((BAR_H - chip_h) / 2.0).floor();
        let mut x = chips_x;
        for (i, w) in widths.iter().enumerate() {
            let (bg, fg) = if i < self.sh.step {
                (theme::CHIP_DONE_BG, theme::LIGHT_GRAY)
            } else if i == self.sh.step {
                if self.sh.in_progress {
                    (
                        Color32::from_rgb(0, (140.0 + pulse * 80.0).min(255.0) as u8, 0),
                        theme::WHITE,
                    )
                } else {
                    (theme::CHIP_CURRENT_BG, theme::WHITE)
                }
            } else {
                (theme::CHIP_UPCOMING_BG, theme::WHITE)
            };
            kit.round_box(x, chip_y, *w, chip_h, 8.0, bg, None);
            kit.text_center(
                x,
                chip_y,
                *w,
                chip_h,
                self.flow.chip(i),
                chip_font.clone(),
                fg,
                None,
            );
            if i + 1 < n {
                kit.text_left(
                    x + w + 5.0,
                    chip_y,
                    chip_h,
                    ">",
                    theme::arial(12.0),
                    theme::GRAY,
                );
            }
            if kit
                .area(&format!("chip{i}"), x, chip_y, *w, chip_h, "")
                .clicked
            {
                self.sh.go(Nav::Chip(i));
            }
            x += w + gap;
        }

        let btn_y = BAR_Y + ((BAR_H - 25.0) / 2.0).floor();
        let chip_right = chips_x + total;
        let left_gap = chips_x - CONTENT_X;
        let right_gap = CONTENT_X + cbw - chip_right;
        let bw = Btn::Small.w();
        let back_x = (CONTENT_X + ((left_gap - bw) / 2.0).floor()).max(CONTENT_X);
        let next_x = (chip_right + ((right_gap - bw) / 2.0).floor()).min(CONTENT_X + cbw - bw);
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

    fn draw_sidebar(&mut self, kit: &mut Kit, panel_y: f32) {
        let px = 15.0;
        kit.text_left_bold(
            px + 8.0,
            panel_y + 12.0,
            20.0,
            &format!("Step {}", self.sh.step + 1),
            theme::conthrax(13.0),
            theme::WHITE,
        );
        let wrap_w = SIDEBAR_W - 16.0;
        let prefix_w = 14.0;
        let mut y = panel_y + 38.0;
        let sc = self.flow.substep_count(self.sh.step);
        for i in 0..sc {
            let name = self.flow.substep_name(self.sh.step, i);
            let (color, glyph) = if i < self.sh.sub {
                (theme::GRAY, 0)
            } else if i == self.sh.sub {
                (theme::CURRENT_GREEN, 1)
            } else {
                (theme::WHITE, 2)
            };
            let font = theme::arial(14.0);
            let mut job = egui::text::LayoutJob::simple(name, font, color, wrap_w - 4.0 - prefix_w);
            job.halign = egui::Align::LEFT;
            let g = kit.ui.ctx().fonts_mut(|f| f.layout_job(job));
            let h = g.size().y.max(22.0);
            let row_x = px + 8.0;
            // Prefix glyph, drawn so it looks the same on every OS.
            let gy = y + 11.0;
            let c = kit.origin + egui::vec2(row_x + 5.0, gy - 11.0 + (22.0 - 0.0) / 2.0);
            match glyph {
                0 => kit.mark(true, color, 11.0, row_x, gy - 6.0),
                1 => {
                    kit.ui.painter().circle_filled(c, 4.5, color);
                }
                _ => {
                    kit.ui
                        .painter()
                        .circle_stroke(c, 4.5, egui::Stroke::new(1.2, color));
                }
            }
            let text_top =
                y + ((22.0 - g.rows.first().map_or(16.0, |r| r.height())) / 2.0).max(0.0);
            kit.ui.painter().galley(
                kit.origin + egui::vec2(row_x + prefix_w, text_top),
                g,
                color,
            );
            if i < self.sh.sub {
                let clicked = kit
                    .hand_area(&format!("side{i}"), row_x, y, wrap_w, h, "")
                    .clicked;
                if clicked {
                    let step = self.sh.step;
                    self.sh.go(Nav::Show(step, i));
                }
            }
            y += h + 4.0;
        }
    }
}

/// Type-erased wizard for the app to hold.
pub trait WizardWindow {
    fn show(
        &mut self,
        ctx: &egui::Context,
        assets: &Assets,
        parent: Option<egui::Rect>,
    ) -> Option<Exit>;
    fn frame(&mut self, ui: &mut egui::Ui, assets: &Assets);
    fn width(&self) -> f32;
    /// Snapshot harness: jump straight to a step.
    fn debug_goto(&mut self, step: usize, sub: usize);
}

impl<F: Flow> WizardWindow for Wizard<F> {
    fn show(
        &mut self,
        ctx: &egui::Context,
        assets: &Assets,
        parent: Option<egui::Rect>,
    ) -> Option<Exit> {
        Wizard::show(self, ctx, assets, parent)
    }

    fn debug_goto(&mut self, step: usize, sub: usize) {
        self.show_step(step, sub);
    }

    fn frame(&mut self, ui: &mut egui::Ui, assets: &Assets) {
        Wizard::frame(self, ui, assets);
    }

    fn width(&self) -> f32 {
        self.sh.fw
    }
}
