//! Absolute-positioned drawing helpers from the installer (images, boxes, text, the
//! ✓/✗ mark and the Metal checkbox). Coordinates are logical pixels from the current
//! origin. The launcher's own widgets are in `style.rs`.

use std::sync::Arc;

use egui::{
    pos2, vec2, Align, Align2, Color32, CornerRadius, FontId, Galley, Id, Pos2, Rect, Sense, Shape,
    Stroke, StrokeKind, TextureHandle, Ui,
};

use super::assets::Assets;
use super::theme::{self, rgba};

pub struct Kit<'a> {
    pub ui: &'a mut Ui,
    pub assets: &'a Assets,
    pub origin: Pos2,
    /// A modal dialog/window is open on top: draw, but don't react.
    pub blocked: bool,
    /// Tip of the widget hovered this frame (the TipBox shows it).
    pub tip: Option<String>,
    id: Id,
}

impl<'a> Kit<'a> {
    pub fn new(
        ui: &'a mut Ui,
        assets: &'a Assets,
        id: impl std::hash::Hash + std::fmt::Debug,
        blocked: bool,
    ) -> Self {
        let origin = ui.max_rect().min;
        Kit {
            ui,
            assets,
            origin,
            blocked,
            tip: None,
            id: Id::new(id),
        }
    }

    /// Runs `f` with painting and interaction clipped to the rect (for scrolling lists).
    pub fn clipped<R>(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        f: impl FnOnce(&mut Kit) -> R,
    ) -> R {
        let saved = self.ui.clip_rect();
        let r = self.rect(x, y, w, h).intersect(saved);
        self.ui.set_clip_rect(r);
        let out = f(self);
        self.ui.set_clip_rect(saved);
        out
    }

    /// Mouse-wheel scrolling over a rect: updates `offset` within `0..=content_h - h`.
    pub fn scroll(&self, x: f32, y: f32, w: f32, h: f32, content_h: f32, offset: &mut f32) {
        let r = self.rect(x, y, w, h);
        if !self.blocked && self.ui.rect_contains_pointer(r) {
            *offset -= self.ui.input(|i| i.smooth_scroll_delta.y);
        }
        *offset = offset.clamp(0.0, (content_h - h).max(0.0));
    }

    pub fn ctx(&self) -> egui::Context {
        self.ui.ctx().clone()
    }

    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.origin + vec2(x, y), vec2(w, h))
    }

    fn painter(&self) -> &egui::Painter {
        self.ui.painter()
    }

    fn wid(&self, key: &str) -> Id {
        self.id.with(key)
    }

    pub fn set_tip(&mut self, tip: &str) {
        if !tip.is_empty() {
            self.tip = Some(tip.to_string());
        }
    }

    // ---- painting ----

    pub fn image(&self, name: &str, x: f32, y: f32, w: f32, h: f32) {
        let tex = self
            .assets
            .tex(self.ui.ctx(), name, w.round() as u32, h.round() as u32);
        self.paint_tex(&tex, self.rect(x, y, w, h), Color32::WHITE);
    }

    fn paint_tex(&self, tex: &TextureHandle, rect: Rect, tint: Color32) {
        self.painter().image(
            tex.id(),
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            tint,
        );
    }

    /// Swing's `fillRoundRect(.., arc, arc)` + 1px `drawRoundRect` border.
    pub fn round_box(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        arc: f32,
        fill: Color32,
        border: Option<Color32>,
    ) {
        let r = self.rect(x, y, w, h);
        let radius = CornerRadius::from(arc / 2.0);
        self.painter().rect_filled(r, radius, fill);
        if let Some(b) = border {
            self.painter()
                .rect_stroke(r, radius, Stroke::new(1.0, b), StrokeKind::Inside);
        }
    }

    pub fn fill(&self, x: f32, y: f32, w: f32, h: f32, color: Color32) {
        self.painter()
            .rect_filled(self.rect(x, y, w, h), 0.0, color);
    }

    /// Lays out `text`, centered per line, wrapping at `wrap` (or never).
    pub fn galley(
        &self,
        text: &str,
        font: FontId,
        color: Color32,
        wrap: Option<f32>,
    ) -> Arc<Galley> {
        let mut job = egui::text::LayoutJob::simple(
            text.to_string(),
            font,
            color,
            wrap.unwrap_or(f32::INFINITY),
        );
        job.halign = Align::Center;
        self.ui.ctx().fonts_mut(|f| f.layout_job(job))
    }

    pub fn text_size(&self, text: &str, font: FontId) -> egui::Vec2 {
        self.galley(text, font, Color32::WHITE, None).size()
    }

    /// Text centered (horizontally and vertically) in a rect, like a centered JLabel.
    pub fn text_center(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        text: &str,
        font: FontId,
        color: Color32,
        wrap: Option<f32>,
    ) {
        let g = self.galley(text, font, color, wrap);
        let r = self.rect(x, y, w, h);
        let top = r.center().y - g.size().y / 2.0;
        self.painter().galley(pos2(r.center().x, top), g, color);
    }

    /// Left-aligned text, vertically centered in a row of height `h`.
    pub fn text_left(&self, x: f32, y: f32, h: f32, text: &str, font: FontId, color: Color32) {
        let r = self.rect(x, y, 0.0, h);
        self.painter().text(
            pos2(r.min.x, r.center().y),
            Align2::LEFT_CENTER,
            text,
            font,
            color,
        );
    }

    /// `markIcon`: vector ✓ / ✗ in a `size` square at (x, y).
    pub fn mark(&self, check: bool, color: Color32, size: f32, x: f32, y: f32) {
        let o = self.origin + vec2(x, y);
        let s = size;
        let stroke = Stroke::new((s / 7.0).max(2.0), color);
        let p = |fx: f32, fy: f32| o + vec2((s * fx).floor(), (s * fy).floor());
        if check {
            self.painter().add(Shape::line(
                vec![p(1.0 / 6.0, 0.5), p(0.4, 0.8), p(5.0 / 6.0, 0.2)],
                stroke,
            ));
            // Round the joint/caps like BasicStroke.CAP_ROUND.
            for q in [p(1.0 / 6.0, 0.5), p(0.4, 0.8), p(5.0 / 6.0, 0.2)] {
                self.painter().circle_filled(q, stroke.width / 2.0, color);
            }
        } else {
            for (a, b) in [(p(0.2, 0.2), p(0.8, 0.8)), (p(0.8, 0.2), p(0.2, 0.8))] {
                self.painter().line_segment([a, b], stroke);
                self.painter().circle_filled(a, stroke.width / 2.0, color);
                self.painter().circle_filled(b, stroke.width / 2.0, color);
            }
        }
    }

    // ---- interaction ----

    /// A Metal-look `JCheckBox` with a Conthrax label, text drawn right of the box. When
    /// `centered`, box+label are centered in the rect (`setHorizontalAlignment(CENTER)`).
    #[allow(clippy::too_many_arguments)]
    pub fn checkbox(
        &mut self,
        key: &str,
        checked: &mut bool,
        label: &str,
        size: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        centered: bool,
        enabled: bool,
        tip: &str,
    ) -> bool {
        let font = theme::conthrax(size);
        let tw = self.text_size(label, font.clone()).x;
        let box_s = 13.0;
        let gap = 4.0;
        let content_w = box_s + gap + tw;
        let bx = if centered {
            x + ((w - content_w) / 2.0).floor()
        } else {
            x + 2.0
        };
        let by = y + ((h - box_s) / 2.0).floor();
        let mut changed = false;
        let mut hovered = false;
        if !self.blocked {
            let resp = self.ui.interact(
                self.rect(bx, y, content_w, h),
                self.wid(key),
                if enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            hovered = resp.hovered();
            if hovered {
                self.set_tip(tip);
            }
            if enabled && resp.clicked() {
                *checked = !*checked;
                changed = true;
            }
        }
        // Metal: white-to-grey box with a dark border, black check.
        let r = self.rect(bx, by, box_s, box_s);
        let fill = if !enabled {
            Color32::from_gray(200)
        } else if hovered {
            Color32::from_rgb(235, 240, 248)
        } else {
            Color32::from_rgb(221, 232, 243)
        };
        self.ui.painter().rect_filled(r, 0.0, fill);
        self.ui.painter().rect_stroke(
            r,
            0.0,
            Stroke::new(1.0, rgba(122, 138, 153, 255)),
            StrokeKind::Inside,
        );
        if *checked {
            let c = if enabled {
                Color32::from_rgb(51, 51, 51)
            } else {
                Color32::from_gray(120)
            };
            self.mark(true, c, box_s, bx, by);
        }
        let color = if enabled {
            theme::WHITE
        } else {
            Color32::from_gray(170)
        };
        self.text_left(bx + box_s + gap, y, h, label, font, color);
        changed
    }
}
