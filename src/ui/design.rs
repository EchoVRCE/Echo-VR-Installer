//! The launcher design (a 1920×1080 concept, drawn at 2/3 in the 1280×720 window):
//! geometry in design pixels, image buttons with shaped hit areas, gradient frames,
//! downloaded textures, letter-spaced text and the icon rail.

use std::sync::Arc;

use egui::epaint::{CornerRadiusF32, PathShape, PathStroke, RectShape};
use egui::text::{LayoutJob, TextFormat};
use egui::{pos2, vec2, Color32, CornerRadius, CursorIcon, Galley, Id, Pos2, Rect, Sense, Shape};

use super::kit::Kit;
use super::style::{self, icon_at, mix, Icon, Resp, ANIM};
use super::theme;

/// Design pixels to logical pixels.
pub const fn dz(px: f32) -> f32 {
    px * (2.0 / 3.0)
}

/// A rect in design pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dr {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Dr {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Dr {
        Dr { x, y, w, h }
    }

    pub fn right(self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(self) -> f32 {
        self.y + self.h
    }

    pub fn shrink(self, d: f32) -> Dr {
        Dr::new(self.x + d, self.y + d, self.w - 2.0 * d, self.h - 2.0 * d)
    }
}

// ---- colours ----

/// Main text on the dark panels.
pub const TEXT: Color32 = Color32::WHITE;
/// Status bar and info line grey.
pub const GREY: Color32 = Color32::from_rgb(124, 122, 128);
/// Discord's body text on the panel.
pub const BODY: Color32 = Color32::from_rgb(190, 188, 202);
/// Discord's `-#` subtext and inactive labels.
pub const SUBTLE: Color32 = Color32::from_rgb(150, 145, 166);
/// Headings and field names on the panel.
pub const HEADING: Color32 = Color32::from_rgb(228, 228, 235);
pub const LINK: Color32 = Color32::from_rgb(82, 112, 222);
/// The status bar: translucent violet over the art.
pub const BAR: Color32 = Color32::from_rgba_unmultiplied_const(40, 9, 138, 107);
pub const QUEST_ON: Color32 = Color32::from_rgb(26, 178, 26);
pub const QUEST_WARN: Color32 = Color32::from_rgb(214, 150, 20);
pub const QUEST_OFF: Color32 = Color32::from_rgb(70, 64, 92);
/// The card rim: violet at the top to pink at the bottom.
pub const RIM_TOP: Color32 = Color32::from_rgb(122, 38, 232);
pub const RIM_BOTTOM: Color32 = Color32::from_rgb(250, 112, 255);
pub const DANGER: Color32 = Color32::from_rgb(255, 96, 96);

/// Even-odd point-in-polygon.
fn inside(p: Pos2, poly: &[Pos2]) -> bool {
    let mut hit = false;
    let mut j = poly.len().wrapping_sub(1);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            hit = !hit;
        }
        j = i;
    }
    hit
}

/// Something to draw in a rail slot.
#[derive(Debug, Clone, Copy)]
pub enum RailIcon {
    /// An embedded image and its height in design pixels.
    Image(&'static str, f32),
    /// A vector icon and its size in design pixels.
    Vector(Icon, f32),
}

impl Kit<'_> {
    pub fn drect(&self, r: Dr) -> Rect {
        self.rect(dz(r.x), dz(r.y), dz(r.w), dz(r.h))
    }

    fn dpos(&self, x: f32, y: f32) -> Pos2 {
        self.origin + vec2(dz(x), dz(y))
    }

    pub fn image_d(&self, name: &str, r: Dr) {
        self.image(name, dz(r.x), dz(r.y), dz(r.w), dz(r.h));
    }

    /// An embedded image multiplied by `tint` (grey = dimmed, alpha = faded).
    pub fn image_tinted(&self, name: &str, r: Dr, tint: Color32) {
        let rect = self.drect(r);
        let tex = self.assets.tex(
            self.ui.ctx(),
            name,
            rect.width().round() as u32,
            rect.height().round() as u32,
        );
        self.paint_tex(&tex, rect, tint);
    }

    /// A downloaded image covering `r` (cropped to its aspect), with rounded corners.
    pub fn texture_cover(&self, tex: &egui::TextureHandle, r: Rect, radius: f32) {
        let [tw, th] = tex.size();
        let (tw, th) = (tw.max(1) as f32, th.max(1) as f32);
        let s = (r.width() / tw).max(r.height() / th);
        let (uw, uh) = (r.width() / (tw * s), r.height() / (th * s));
        let uv = Rect::from_min_size(pos2((1.0 - uw) / 2.0, (1.0 - uh) / 2.0), vec2(uw, uh));
        self.ui.painter().add(
            RectShape::filled(r, CornerRadius::from(radius), Color32::WHITE)
                .with_texture(tex.id(), uv),
        );
    }

    /// A rounded rim whose colour runs from `top` to `bottom`.
    pub fn gradient_frame(&self, r: Rect, radius: f32, width: f32, top: Color32, bottom: Color32) {
        let mut points = Vec::new();
        egui::epaint::tessellator::path::rounded_rectangle(
            &mut points,
            r.shrink(width / 2.0),
            CornerRadiusF32::same(radius),
        );
        let (y0, h) = (r.min.y, r.height().max(1.0));
        let stroke = PathStroke::new_uv(width, move |_, p| mix(top, bottom, (p.y - y0) / h));
        self.ui.painter().add(Shape::Path(PathShape {
            points,
            closed: true,
            fill: Color32::TRANSPARENT,
            stroke,
        }));
    }

    /// Single-line text with extra letter spacing (and an optional underline).
    pub fn spaced_galley(
        &self,
        text: &str,
        font: egui::FontId,
        color: Color32,
        spacing: f32,
        underline: bool,
    ) -> Arc<Galley> {
        let mut job = LayoutJob::default();
        job.append(
            text,
            0.0,
            TextFormat {
                font_id: font,
                color,
                extra_letter_spacing: spacing,
                underline: if underline {
                    egui::Stroke::new(1.0, color)
                } else {
                    egui::Stroke::NONE
                },
                ..Default::default()
            },
        );
        self.ui.ctx().fonts_mut(|f| f.layout_job(job))
    }

    /// Draws a galley with its top-left at logical (x, y); returns where it went.
    pub fn put(&self, x: f32, y: f32, g: Arc<Galley>) -> Rect {
        let r = Rect::from_min_size(self.origin + vec2(x, y), g.size());
        self.ui.painter().galley(r.min, g, TEXT);
        r
    }

    /// Hover/click inside `shape` (a polygon in design pixels), reacting only within
    /// `area`. Buttons whose images overlap (PLAY's slant under CHECK FOR UPDATES') get
    /// areas that don't, and their shapes keep the empty corners inert.
    pub fn hot_shape(
        &mut self,
        key: &str,
        area: Dr,
        shape: &[(f32, f32)],
        enabled: bool,
        tip: &str,
    ) -> (Resp, f32, bool) {
        let id = Id::new(("design", key));
        let poly: Vec<Pos2> = shape.iter().map(|&(x, y)| self.dpos(x, y)).collect();
        let mut out = Resp::default();
        let mut pressed = false;
        if !self.blocked {
            let sense = if enabled {
                Sense::click()
            } else {
                Sense::hover()
            };
            let resp = self.ui.interact(self.drect(area), id, sense);
            let within = |p: Option<Pos2>| p.is_some_and(|p| inside(p, &poly));
            out.hovered = resp.hovered() && within(resp.hover_pos());
            if enabled {
                out.clicked = resp.clicked() && within(resp.interact_pointer_pos());
                pressed = out.hovered && resp.is_pointer_button_down_on();
                if out.hovered {
                    self.ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
            }
            if out.hovered && !tip.is_empty() {
                resp.on_hover_text(tip);
            }
        }
        let t =
            self.ui
                .ctx()
                .animate_bool_with_time(id.with("hover"), out.hovered && enabled, ANIM);
        (out, t, pressed)
    }

    /// Hover and press feedback over an image button: a light or dark veil in its shape.
    pub fn shape_veil(&self, shape: &[(f32, f32)], t: f32, pressed: bool) {
        let color = if pressed {
            Color32::from_black_alpha(50)
        } else {
            Color32::from_white_alpha((34.0 * t) as u8)
        };
        if pressed || t > 0.01 {
            let poly = shape.iter().map(|&(x, y)| self.dpos(x, y)).collect();
            self.ui
                .painter()
                .add(Shape::convex_polygon(poly, color, egui::Stroke::NONE));
        }
    }

    /// A rail slot centred at design y `cy`: the blue glow square when selected, a soft
    /// square on hover, and the icon.
    pub fn rail_item(
        &mut self,
        key: &str,
        icon: RailIcon,
        cy: f32,
        selected: bool,
        tip: &str,
    ) -> bool {
        let sq = Dr::new(21.0, cy - 26.5, 52.0, 53.0);
        let (resp, t, _) = self.hot(key, self.drect(sq), true, tip);
        if selected {
            // sidebar_selected.png: a 264 px square at (74, 74) in its 412×416 glow.
            let s = sq.w / 264.0;
            self.image_d(
                "sidebar_selected.png",
                Dr::new(sq.x - 74.0 * s, sq.y - 74.0 * s, 412.0 * s, 416.0 * s),
            );
        } else if t > 0.01 {
            self.image_tinted(
                "sidebar_hover.png",
                sq,
                Color32::from_white_alpha((150.0 * t) as u8),
            );
        }
        let (cx, cy) = (47.0, cy);
        match icon {
            RailIcon::Image(name, h) => {
                let (nw, nh) = super::assets::native_size(name);
                let w = h * nw as f32 / nh.max(1) as f32;
                self.image_d(name, Dr::new(cx - w / 2.0, cy - h / 2.0, w, h));
            }
            RailIcon::Vector(i, s) => {
                let o = self.dpos(cx - s / 2.0, cy - s / 2.0);
                let color = if selected {
                    TEXT
                } else {
                    mix(Color32::from_gray(225), TEXT, t)
                };
                icon_at(self.ui.painter(), i, o, dz(s), color);
            }
        }
        resp.clicked
    }

    /// Clicks on something already drawn at `r` (links): returns the click.
    pub fn click_area(&mut self, key: &str, r: Rect, tip: &str) -> bool {
        self.hot(key, r, true, tip).0.clicked
    }
}

/// DIN caps (status bar, info line, card text).
pub fn din(size_design: f32) -> egui::FontId {
    theme::din(dz(size_design))
}

pub fn myriad(size_design: f32) -> egui::FontId {
    theme::myriad(dz(size_design))
}

pub fn myriad_bold(size_design: f32) -> egui::FontId {
    theme::myriad_bold(dz(size_design))
}

pub fn conthrax(size_design: f32) -> egui::FontId {
    style::display(dz(size_design))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygon_hit_test() {
        let tri = [pos2(0.0, 0.0), pos2(10.0, 0.0), pos2(0.0, 10.0)];
        assert!(inside(pos2(2.0, 2.0), &tri));
        assert!(!inside(pos2(8.0, 8.0), &tri));
        assert!(!inside(pos2(-1.0, 2.0), &tri));
    }
}
