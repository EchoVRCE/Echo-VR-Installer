//! The window frame shared by the launcher and the installer wizards: background art,
//! the blue status bar (with its pulse), the wine sidebar box, the big magenta section
//! box, the wine bottom bar, the sidebar step list and the step chips.
//!
//! Everything is laid out on the wizard's 1056x594 canvas; the window shows it at
//! 1280x720 through egui's zoom factor (`theme::SCALE`).

use std::time::Instant;

use egui::Color32;

use super::kit::Kit;
use super::theme;

pub const W: f32 = 1056.0;
pub const FH: f32 = 594.0;
pub const SIDEBAR_W: f32 = 120.0;
pub const CONTENT_X: f32 = SIDEBAR_W + 30.0; // 150
pub const CONTENT_W: f32 = W - CONTENT_X - 10.0; // 896
pub const SECTION_Y: f32 = 52.0;
pub const SECTION_H: f32 = 450.0;
pub const BAR_Y: f32 = FH - 74.0; // 520
pub const BAR_H: f32 = 42.0;
const CHIP_H: f32 = 24.0;
const CHIP_GAP: f32 = 12.0;

/// The status bar's pulse: `phase += 0.15` every 50 ms while work runs.
pub struct Pulse {
    phase: f32,
    last: Instant,
}

impl Default for Pulse {
    fn default() -> Self {
        Pulse {
            phase: 0.0,
            last: Instant::now(),
        }
    }
}

impl Pulse {
    /// Advances the phase while `active`; returns the pulse amount (0..1).
    pub fn tick(&mut self, active: bool) -> f32 {
        let now = Instant::now();
        if active {
            self.phase += 0.15 * (now - self.last).as_millis() as f32 / 50.0;
        }
        self.last = now;
        self.phase.sin() * 0.5 + 0.5
    }
}

/// The status bar colour while work runs, at pulse amount `p`.
pub fn pulse_fill(p: f32) -> Color32 {
    Color32::from_rgb(
        (50.0 + p * 40.0) as u8,
        (90.0 + p * 50.0) as u8,
        (150.0 + p * 60.0) as u8,
    )
}

pub fn background(kit: &Kit, img: &str) {
    kit.image(img, 0.0, 0.0, W, FH);
}

/// The sidebar box and the big section box.
pub fn boxes(kit: &Kit) {
    kit.section_box(
        10.0,
        SECTION_Y,
        SIDEBAR_W + 10.0,
        SECTION_H,
        15.0,
        theme::SIDEBAR_FILL,
    );
    kit.section_box(
        CONTENT_X,
        SECTION_Y,
        CONTENT_W,
        SECTION_H,
        15.0,
        theme::SECTION_FILL,
    );
}

pub fn status_bar(kit: &Kit, text: &str, fill: Color32) {
    kit.round_box(
        CONTENT_X,
        10.0,
        CONTENT_W,
        32.0,
        8.0,
        fill,
        Some(theme::BOX_BORDER),
    );
    kit.text_center(
        CONTENT_X,
        10.0,
        CONTENT_W,
        32.0,
        text,
        theme::arial_bold(14.0),
        theme::WHITE,
        None,
    );
}

pub fn bottom_bar(kit: &Kit) {
    kit.section_box(
        CONTENT_X,
        BAR_Y,
        CONTENT_W,
        BAR_H,
        15.0,
        theme::SIDEBAR_FILL,
    );
}

/// Top of a `Btn::Small` centered in the bottom bar.
pub fn bar_button_y() -> f32 {
    BAR_Y + ((BAR_H - 25.0) / 2.0).floor()
}

// ---- sidebar list ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// Grey ✓.
    Done,
    /// Green dot, green text.
    Current,
    /// White ring.
    Open,
}

pub struct SideRow<'a> {
    pub label: &'a str,
    pub mark: Mark,
    pub clickable: bool,
    pub tip: &'a str,
}

/// The sidebar's list: a Conthrax heading ("Step 2") and rows with a ✓ / ● / ○ glyph.
/// Returns the clicked row and the y just below the list.
pub fn side_list(
    kit: &mut Kit,
    key: &str,
    y: f32,
    heading: &str,
    rows: &[SideRow],
) -> (Option<usize>, f32) {
    let px = 15.0;
    kit.text_left_bold(
        px + 8.0,
        y + 12.0,
        20.0,
        heading,
        theme::conthrax(13.0),
        theme::WHITE,
    );
    let wrap_w = SIDEBAR_W - 16.0;
    let prefix_w = 14.0;
    let row_x = px + 8.0;
    let mut y = y + 38.0;
    let mut clicked = None;
    for (i, row) in rows.iter().enumerate() {
        let font = theme::arial(14.0);
        let measure = egui::text::LayoutJob::simple(
            row.label.to_string(),
            font.clone(),
            theme::WHITE,
            wrap_w - 4.0 - prefix_w,
        );
        let h = kit
            .ui
            .ctx()
            .fonts_mut(|f| f.layout_job(measure))
            .size()
            .y
            .max(22.0);
        let mut hovered = false;
        if row.clickable {
            let c = kit.hand_area(&format!("{key}{i}"), row_x, y, wrap_w, h, row.tip);
            hovered = c.hovered;
            if c.clicked {
                clicked = Some(i);
            }
        }
        let (color, glyph) = match row.mark {
            Mark::Done if hovered => (theme::WHITE, Mark::Done),
            Mark::Done => (theme::GRAY, Mark::Done),
            Mark::Current => (theme::CURRENT_GREEN, Mark::Current),
            Mark::Open if hovered => (theme::HOVER_GREEN, Mark::Open),
            Mark::Open => (theme::WHITE, Mark::Open),
        };
        let mut job = egui::text::LayoutJob::simple(
            row.label.to_string(),
            font,
            color,
            wrap_w - 4.0 - prefix_w,
        );
        job.halign = egui::Align::LEFT;
        let g = kit.ui.ctx().fonts_mut(|f| f.layout_job(job));
        // Prefix glyph, drawn so it looks the same on every OS.
        let c = kit.origin + egui::vec2(row_x + 5.0, y + 11.0);
        match glyph {
            Mark::Done => kit.mark(true, color, 11.0, row_x, y + 5.0),
            Mark::Current => {
                kit.ui.painter().circle_filled(c, 4.5, color);
            }
            Mark::Open => {
                kit.ui
                    .painter()
                    .circle_stroke(c, 4.5, egui::Stroke::new(1.2, color));
            }
        }
        let text_top = y + ((22.0 - g.rows.first().map_or(16.0, |r| r.height())) / 2.0).max(0.0);
        kit.ui.painter().galley(
            kit.origin + egui::vec2(row_x + prefix_w, text_top),
            g,
            color,
        );
        y += h + 4.0;
    }
    (clicked, y)
}

/// A single grey sidebar link pinned to the bottom of the sidebar box ("< Launcher").
pub fn side_link(kit: &mut Kit, key: &str, text: &str, tip: &str) -> bool {
    let (x, h) = (23.0, 22.0);
    let y = SECTION_Y + SECTION_H - 12.0 - h;
    let w = SIDEBAR_W - 16.0;
    let c = kit.hand_area(key, x, y, w, h, tip);
    let color = if c.hovered {
        theme::WHITE
    } else {
        theme::LIGHT_GRAY
    };
    kit.text_left(x, y, h, text, theme::arial(14.0), color);
    c.clicked
}

// ---- chips ----

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Chip {
    Done,
    Current,
    /// Current while work runs: the green pulses (pulse amount 0..1).
    Busy(f32),
    Upcoming,
}

fn chip_font() -> egui::FontId {
    theme::conthrax(9.0)
}

/// Chip widths and the row's total width.
pub fn chip_widths(kit: &Kit, labels: &[&str]) -> (Vec<f32>, f32) {
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| (kit.text_size(l, chip_font()).x.round() + 16.0).clamp(40.0, 74.0))
        .collect();
    let total = widths.iter().sum::<f32>() + CHIP_GAP * labels.len().saturating_sub(1) as f32;
    (widths, total)
}

/// The step chips from `x`, vertically centered in the bottom bar. `arrows` draws the
/// `>` separators of the wizards. Returns the clicked chip.
#[allow(clippy::too_many_arguments)]
pub fn chips(
    kit: &mut Kit,
    key: &str,
    x: f32,
    labels: &[&str],
    states: &[Chip],
    arrows: bool,
    tips: &[&str],
) -> Option<usize> {
    let (widths, _) = chip_widths(kit, labels);
    let y = BAR_Y + ((BAR_H - CHIP_H) / 2.0).floor();
    let mut x = x;
    let mut clicked = None;
    for (i, w) in widths.iter().enumerate() {
        let state = states.get(i).copied().unwrap_or(Chip::Upcoming);
        let tip = tips.get(i).copied().unwrap_or("");
        let c = kit.area(&format!("{key}{i}"), x, y, *w, CHIP_H, tip);
        let current = matches!(state, Chip::Current | Chip::Busy(_));
        if c.hovered && !current {
            kit.ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let (bg, fg) = match state {
            Chip::Done if c.hovered => (theme::CHIP_HOVER_BG, theme::WHITE),
            Chip::Done => (theme::CHIP_DONE_BG, theme::LIGHT_GRAY),
            Chip::Current => (theme::CHIP_CURRENT_BG, theme::WHITE),
            Chip::Busy(p) => (
                Color32::from_rgb(0, (140.0 + p * 80.0).min(255.0) as u8, 0),
                theme::WHITE,
            ),
            Chip::Upcoming if c.hovered => (theme::CHIP_HOVER_BG, theme::WHITE),
            Chip::Upcoming => (theme::CHIP_UPCOMING_BG, theme::WHITE),
        };
        kit.round_box(x, y, *w, CHIP_H, 8.0, bg, None);
        kit.text_center(x, y, *w, CHIP_H, labels[i], chip_font(), fg, None);
        if arrows && i + 1 < labels.len() {
            kit.text_left(x + w + 5.0, y, CHIP_H, ">", theme::arial(12.0), theme::GRAY);
        }
        if c.clicked {
            clicked = Some(i);
        }
        x += w + CHIP_GAP;
    }
    clicked
}

/// A chip that only shows a state (badges like "Selected"); returns its width.
pub fn badge(kit: &Kit, x: f32, y: f32, text: &str, on: bool) -> f32 {
    let w = kit.text_size(text, chip_font()).x.round() + 16.0;
    let bg = if on {
        theme::CHIP_CURRENT_BG
    } else {
        theme::CHIP_DONE_BG
    };
    kit.round_box(x, y, w, 20.0, 8.0, bg, None);
    kit.text_center(x, y, w, 20.0, text, chip_font(), theme::WHITE, None);
    w
}

pub fn badge_width(kit: &Kit, text: &str) -> f32 {
    kit.text_size(text, chip_font()).x.round() + 16.0
}
