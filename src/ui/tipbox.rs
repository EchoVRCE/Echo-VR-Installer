//! The Clippy easter egg from the installer's TipBox (double-click the About logo).

use std::time::{Duration, Instant};

use egui::{pos2, vec2, Color32, Rect};

use super::kit::Kit;

const TICK: Duration = Duration::from_millis(80);
const RISE_FRAMES: usize = 10;
const FALL_FRAMES: usize = 10;
const HOLD: Duration = Duration::from_millis(2000);
const CLIPPY_W: f32 = 124.0;
const CLIPPY_H: f32 = 93.0;

/// The Clippy easter egg: rises from behind an edge, idles, and sinks back.
#[derive(Default)]
pub struct Clippy {
    start: Option<Instant>,
}

impl Clippy {
    /// Starts the animation (ignored while one is running).
    pub fn trigger(&mut self, ctx: &egui::Context) {
        if self.start.is_none() {
            self.start = Some(Instant::now());
            ctx.request_repaint();
        }
    }

    /// Draws Clippy rising from `y` (the top edge it hides behind), centered on `w`.
    pub fn draw(&mut self, kit: &mut Kit, x: f32, y: f32, w: f32) {
        let Some(start) = self.start else {
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
            self.start = None;
            return;
        };
        let cx = x + ((w - CLIPPY_W) / 2.0).floor();
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
}
