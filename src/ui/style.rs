//! The launcher's few widgets beyond the installer's own (`kit.rs`, `frame.rs`):
//! dropdowns and menus on the slanted buttons, the progress label with a green fill,
//! and hover tips drawn as a small TipBox. No new visual language -- only the
//! installer's parts, colours and fonts.

use std::time::{Duration, Instant};

use egui::{pos2, vec2, Color32, CursorIcon, Id, Order, Rect, Sense, Stroke, StrokeKind};

use super::kit::{Btn, Kit};
use super::theme::{self, rgba};

pub const TEXT: Color32 = Color32::WHITE;
/// Secondary text on the magenta section box.
pub const TEXT_DIM: Color32 = Color32::from_rgb(230, 225, 235);
/// Popup rows under the pointer.
const ROW_HOVER: Color32 = rgba(200, 0, 150, 150);
/// The TipBox's frame and body colours.
const TIP_FRAME: Color32 = rgba(200, 0, 150, 230);
const TIP_BODY: Color32 = rgba(70, 70, 70, 235);
const TIP_DELAY: Duration = Duration::from_millis(450);

pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

impl Kit<'_> {
    fn sid(&self, key: &str) -> Id {
        Id::new(("style", key))
    }

    pub fn text(&self, x: f32, y: f32, text: &str, font: egui::FontId, color: Color32) {
        self.ui.painter().text(
            self.origin + vec2(x, y),
            egui::Align2::LEFT_TOP,
            text,
            font,
            color,
        );
    }

    /// Left-aligned text clipped to `w` with an ellipsis.
    pub fn text_fit(&self, x: f32, y: f32, w: f32, text: &str, font: egui::FontId, color: Color32) {
        let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
        job.wrap = egui::text::TextWrapping::truncate_at_width(w);
        let g = self.ui.ctx().fonts_mut(|f| f.layout_job(job));
        self.ui.painter().galley(self.origin + vec2(x, y), g, color);
    }

    /// Centered text on one line, clipped to `w` with an ellipsis.
    #[allow(clippy::too_many_arguments)]
    pub fn text_fit_center(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        text: &str,
        font: egui::FontId,
        color: Color32,
    ) {
        let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
        job.wrap = egui::text::TextWrapping::truncate_at_width(w);
        let g = self.ui.ctx().fonts_mut(|f| f.layout_job(job));
        let r = self.rect(x, y, w, h);
        let pos = pos2(
            r.center().x - g.size().x / 2.0,
            r.center().y - g.size().y / 2.0,
        );
        self.ui.painter().galley(pos, g, color);
    }

    pub fn text_width(&self, text: &str, font: egui::FontId) -> f32 {
        self.ui
            .ctx()
            .fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, TEXT).size().x)
    }

    /// The installer's progress label (white box, black Conthrax) with a green fill for
    /// the fraction, or a sliding green segment when unknown.
    #[allow(clippy::too_many_arguments)]
    pub fn progress(&self, x: f32, y: f32, w: f32, h: f32, fraction: Option<f32>, label: &str) {
        let r = self.rect(x, y, w, h);
        self.ui.painter().rect_filled(r, 0.0, theme::PROGRESS_BG);
        match fraction {
            Some(f) => {
                let fill = Rect::from_min_size(r.min, vec2(w * f.clamp(0.0, 1.0), r.height()));
                self.ui
                    .painter()
                    .rect_filled(fill, 0.0, with_alpha(theme::STATUS_DONE, 200));
            }
            None => {
                let t = self.ui.input(|i| i.time) as f32;
                let seg = w * 0.25;
                let pos = ((t * 0.8).fract() * (w + seg)) - seg;
                let a = (r.min.x + pos).max(r.min.x);
                let b = (r.min.x + pos + seg).min(r.max.x);
                if b > a {
                    self.ui.painter().rect_filled(
                        Rect::from_x_y_ranges(a..=b, r.y_range()),
                        0.0,
                        with_alpha(theme::STATUS_DONE, 170),
                    );
                }
                self.ui.ctx().request_repaint();
            }
        }
        let mut job = egui::text::LayoutJob::simple_singleline(
            label.to_string(),
            theme::conthrax(11.0),
            theme::BLACK,
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(w - 12.0);
        let g = self.ui.ctx().fonts_mut(|f| f.layout_job(job));
        let pos = pos2(
            r.center().x - g.size().x / 2.0,
            r.center().y - g.size().y / 2.0,
        );
        self.ui.painter().galley(pos, g, theme::BLACK);
    }

    // ---- dropdowns / menus ----

    fn toggle_open(&self, key: &str) {
        let open_id = self.sid(key).with("open");
        let open = self
            .ui
            .ctx()
            .data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
        self.ui.ctx().data_mut(|d| d.insert_temp(open_id, !open));
    }

    /// A slanted button showing `options[selected]` and a chevron; returns a new pick.
    #[allow(clippy::too_many_arguments)]
    pub fn dropdown(
        &mut self,
        key: &str,
        kind: Btn,
        options: &[String],
        selected: usize,
        x: f32,
        y: f32,
        w: f32,
        tip: &str,
    ) -> Option<usize> {
        let label = options.get(selected).cloned().unwrap_or_default();
        let b = self.button_frame(key, kind, x, y, w, !options.is_empty(), tip);
        let h = kind.size().1;
        let size = if kind == Btn::Small { 11.0 } else { 12.0 };
        self.text_fit_center(
            x + 14.0,
            y,
            w - 44.0,
            h,
            &label,
            theme::conthrax(size),
            b.text_color,
        );
        chevron(
            self.ui.painter(),
            b.rect.right_center() - vec2(24.0, 0.0),
            b.text_color,
        );
        if b.clicked {
            self.toggle_open(key);
            return None;
        }
        let entries: Vec<(String, bool)> = options
            .iter()
            .enumerate()
            .map(|(i, o)| (o.clone(), i == selected))
            .collect();
        self.menu_popup(key, b.rect, w.max(200.0), &entries)
            .filter(|i| *i != selected)
    }

    /// A small slanted button that opens a menu of actions; returns the picked index.
    #[allow(clippy::too_many_arguments)]
    pub fn menu_button(
        &mut self,
        key: &str,
        label: &str,
        items: &[&str],
        x: f32,
        y: f32,
        w: f32,
        tip: &str,
    ) -> Option<usize> {
        let b = self.button_frame(key, Btn::Small, x, y, w, true, tip);
        self.text_fit_center(
            x + 8.0,
            y,
            w - 30.0,
            25.0,
            label,
            theme::conthrax(11.0),
            b.text_color,
        );
        chevron(
            self.ui.painter(),
            b.rect.right_center() - vec2(16.0, 0.0),
            b.text_color,
        );
        if b.clicked {
            self.toggle_open(key);
            return None;
        }
        let entries: Vec<(String, bool)> = items.iter().map(|s| (s.to_string(), false)).collect();
        let pw = 200.0;
        let anchor = Rect::from_min_size(pos2(b.rect.max.x - pw, b.rect.min.y), vec2(pw, 25.0));
        self.menu_popup(key, anchor, pw, &entries)
    }

    /// The popup list under `anchor` while `key` is open: a wine panel with magenta hover
    /// rows and the sidebar's green dot on the current entry. Closes on pick, outside
    /// click or Escape.
    fn menu_popup(
        &mut self,
        key: &str,
        anchor: Rect,
        w: f32,
        entries: &[(String, bool)],
    ) -> Option<usize> {
        let open_id = self.sid(key).with("open");
        let open = self
            .ui
            .ctx()
            .data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
        if !open || self.blocked {
            return None;
        }
        let row = 26.0;
        let h = entries.len() as f32 * row + 8.0;
        let screen = self.ui.ctx().content_rect();
        let below = anchor.max.y + 4.0 + h <= screen.max.y;
        let pos = if below {
            pos2(anchor.min.x, anchor.max.y + 4.0)
        } else {
            pos2(anchor.min.x, anchor.min.y - 4.0 - h)
        };
        let popup = Rect::from_min_size(pos, vec2(w, h));
        let mut picked = None;
        let ctx = self.ui.ctx().clone();
        egui::Area::new(open_id.with("area"))
            .order(Order::Foreground)
            .fixed_pos(pos)
            .show(&ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
                let p = ui.painter();
                p.rect_filled(rect, 7.5, rgba(100, 0, 50, 245));
                p.rect_stroke(
                    rect,
                    7.5,
                    Stroke::new(1.0, theme::BOX_BORDER),
                    StrokeKind::Inside,
                );
                for (i, (label, current)) in entries.iter().enumerate() {
                    let rr = Rect::from_min_size(
                        pos2(rect.min.x + 4.0, rect.min.y + 4.0 + i as f32 * row),
                        vec2(w - 8.0, row),
                    );
                    let resp = ui.interact(rr, open_id.with(i), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(rr, 4.0, ROW_HOVER);
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    let color = if *current { theme::CURRENT_GREEN } else { TEXT };
                    if *current {
                        ui.painter().circle_filled(
                            pos2(rr.min.x + 12.0, rr.center().y),
                            4.5,
                            theme::CURRENT_GREEN,
                        );
                    }
                    let mut job = egui::text::LayoutJob::simple_singleline(
                        label.clone(),
                        theme::arial(14.0),
                        color,
                    );
                    job.wrap = egui::text::TextWrapping::truncate_at_width(w - 40.0);
                    let g = ui.ctx().fonts_mut(|f| f.layout_job(job));
                    ui.painter().galley(
                        pos2(rr.min.x + 24.0, rr.center().y - g.size().y / 2.0),
                        g,
                        color,
                    );
                    if resp.clicked() {
                        picked = Some(i);
                    }
                }
            });
        let outside_click = ctx.input(|i| {
            i.pointer.any_click()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !popup.contains(p) && !anchor.contains(p))
        });
        if picked.is_some() || outside_click || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.data_mut(|d| d.insert_temp(open_id, false));
        }
        picked
    }
}

fn chevron(p: &egui::Painter, c: egui::Pos2, color: Color32) {
    p.add(egui::Shape::line(
        vec![
            c + vec2(-5.0, -2.5),
            c + vec2(0.0, 2.5),
            c + vec2(5.0, -2.5),
        ],
        Stroke::new(1.8, color),
    ));
}

/// Hover tips as a small TipBox (magenta frame, grey body, white Arial) near the pointer,
/// once the pointer rested on a widget for a moment.
#[derive(Default)]
pub struct Tips {
    current: Option<(String, Instant)>,
}

impl Tips {
    /// Call once per frame, last, with the tip the kit saw hovered.
    pub fn show(&mut self, ctx: &egui::Context, tip: Option<String>) {
        let Some(tip) = tip else {
            self.current = None;
            return;
        };
        let since = match &self.current {
            Some((t, s)) if *t == tip => *s,
            _ => {
                let now = Instant::now();
                self.current = Some((tip.clone(), now));
                now
            }
        };
        let waited = since.elapsed();
        if waited < TIP_DELAY {
            ctx.request_repaint_after(TIP_DELAY - waited);
            return;
        }
        let Some(ptr) = ctx.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        let g = ctx.fonts_mut(|f| {
            let mut job =
                egui::text::LayoutJob::simple(tip.clone(), theme::arial(13.0), TEXT, 260.0);
            job.halign = egui::Align::Center;
            f.layout_job(job)
        });
        let size = g.size() + vec2(30.0, 24.0);
        let screen = ctx.content_rect();
        let mut pos = ptr + vec2(14.0, 20.0);
        if pos.x + size.x > screen.max.x - 4.0 {
            pos.x = (ptr.x - 14.0 - size.x).max(screen.min.x + 4.0);
        }
        if pos.y + size.y > screen.max.y - 4.0 {
            pos.y = ptr.y - 8.0 - size.y;
        }
        egui::Area::new(Id::new("launcher-tip"))
            .order(Order::Tooltip)
            .fixed_pos(pos)
            .interactable(false)
            .show(ctx, |ui| {
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                let p = ui.painter();
                p.rect_filled(rect, 7.5, TIP_FRAME);
                p.rect_stroke(
                    rect,
                    7.5,
                    Stroke::new(1.0, rgba(50, 50, 50, 255)),
                    StrokeKind::Inside,
                );
                let body = rect.shrink(5.0);
                p.rect_filled(body, 4.0, TIP_BODY);
                p.rect_stroke(
                    body,
                    4.0,
                    Stroke::new(1.0, rgba(50, 50, 50, 200)),
                    StrokeKind::Inside,
                );
                p.galley(
                    pos2(body.center().x, body.center().y - g.size().y / 2.0),
                    g,
                    TEXT,
                );
            });
    }
}
