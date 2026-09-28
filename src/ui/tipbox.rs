//! The TipBox (316x147) and its Clippy easter egg (double-click the box).

use std::time::{Duration, Instant};

use egui::{pos2, vec2, Color32, Rect};

use super::kit::Kit;
use super::theme::{self, rgba};

pub const W: f32 = 316.0;
pub const H: f32 = 147.0;
const DEFAULT_TEXT: &str = "Hover over items for tips.";

const TICK: Duration = Duration::from_millis(80);
const RISE_FRAMES: usize = 10;
const FALL_FRAMES: usize = 10;
const HOLD: Duration = Duration::from_millis(2000);
const CLIPPY_W: f32 = 124.0;
const CLIPPY_H: f32 = 93.0;

#[derive(Default)]
pub struct TipBox {
    /// A tip set by an action (not a hover); shown until the pointer next leaves a widget.
    sticky: Option<String>,
    shown: Option<String>,
    was_hovering: bool,
    clippy_start: Option<Instant>,
}

impl TipBox {
    /// Shows `tip` until the user hovers something else (Swing `showTip` without a hover).
    pub fn show_tip(&mut self, tip: &str) {
        self.sticky = Some(tip.to_string());
    }

    /// Decides this frame's text from what the kit saw hovered.
    fn update(&mut self, hovered_tip: Option<String>) {
        match hovered_tip {
            Some(t) => {
                self.shown = Some(t);
                self.sticky = None;
                self.was_hovering = true;
            }
            None if self.was_hovering => {
                self.shown = None;
                self.was_hovering = false;
            }
            None => self.shown = self.sticky.clone(),
        }
    }

    /// Clippy rises out from *behind* the box, so this is drawn before everything else.
    pub fn draw_clippy(&mut self, kit: &mut Kit, x: f32, y: f32) {
        let Some(start) = self.clippy_start else {
            return;
        };
        let frames = kit.assets.clippy_frames(kit.ui.ctx());
        let n = frames.len();
        let elapsed = start.elapsed();
        let rise = TICK * RISE_FRAMES as u32;
        let fall = TICK * FALL_FRAMES as u32;
        let (start_y, target_y) = (y, (y - CLIPPY_H).max(0.0));
        let ticks = |d: Duration| (d.as_millis() / TICK.as_millis()) as usize;
        let (frame, cy) = if elapsed < rise {
            let p = elapsed.as_secs_f32() / rise.as_secs_f32();
            (
                ticks(elapsed).min(RISE_FRAMES - 1),
                start_y - ((start_y - target_y) * p).floor(),
            )
        } else if elapsed < rise + HOLD {
            let middle = n.saturating_sub(RISE_FRAMES + FALL_FRAMES).max(1);
            let mut f = RISE_FRAMES + ticks(elapsed - rise) % middle;
            if f >= n.saturating_sub(FALL_FRAMES) {
                f = RISE_FRAMES;
            }
            (f, target_y)
        } else if elapsed < rise + HOLD + fall {
            let e = elapsed - rise - HOLD;
            let p = e.as_secs_f32() / fall.as_secs_f32();
            let fi = ticks(e).min(FALL_FRAMES - 1);
            (
                n.saturating_sub(FALL_FRAMES) + fi,
                target_y + ((start_y - target_y) * p).floor(),
            )
        } else {
            self.clippy_start = None;
            return;
        };
        let cx = x + ((W - CLIPPY_W) / 2.0).floor();
        let rect = Rect::from_min_size(kit.origin + vec2(cx, cy), vec2(CLIPPY_W, CLIPPY_H));
        if let Some(tex) = frames.get(frame.min(n.saturating_sub(1))) {
            kit.ui.painter().image(
                tex.id(),
                rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            kit.ui.painter().rect_stroke(
                rect,
                0.0,
                egui::Stroke::new(1.0, Color32::RED),
                egui::StrokeKind::Inside,
            );
        }
        kit.ui.ctx().request_repaint_after(TICK);
    }

    /// Draws the box at (x, y) with the tip of whatever the kit saw hovered this frame.
    /// Call after all tip-bearing widgets of the frame.
    pub fn draw(&mut self, kit: &mut Kit, x: f32, y: f32) {
        let hovered = kit.tip.take();
        self.update(hovered);

        kit.round_box(
            x,
            y,
            W,
            H,
            15.0,
            rgba(200, 0, 150, 200),
            Some(rgba(50, 50, 50, 255)),
        );
        kit.image("tipbox_top.png", x + 8.0, y + 8.0, 300.0, 26.0);
        kit.text_center(
            x + 8.0,
            y + 8.0,
            300.0,
            26.0,
            "Tipbox",
            theme::conthrax(20.0),
            theme::WHITE,
            None,
        );
        kit.round_box(
            x + 16.0,
            y + 39.0,
            284.0,
            100.0,
            8.0,
            rgba(70, 70, 70, 180),
            Some(rgba(50, 50, 50, 200)),
        );
        let text = self.shown.as_deref().unwrap_or(DEFAULT_TEXT);
        kit.text_center(
            x + 16.0,
            y + 39.0,
            284.0,
            100.0,
            text,
            theme::arial(14.0),
            theme::WHITE,
            Some(278.0),
        );

        if !kit.blocked {
            let rect = kit.rect(x, y, W, H);
            let resp = kit
                .ui
                .interact(rect, egui::Id::new("tipbox-clippy"), egui::Sense::click());
            if resp.double_clicked() && self.clippy_start.is_none() {
                self.clippy_start = Some(Instant::now());
                kit.ui.ctx().request_repaint();
            }
        }
    }
}
