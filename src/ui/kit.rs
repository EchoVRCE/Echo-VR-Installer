//! Absolute-positioned drawing helpers that recreate the Swing custom widgets
//! (`SpecialButton`, `SpecialLabel`, `SpecialTextfield`, `SpecialCheckBox`, ...).
//!
//! Coordinates are Swing's: logical pixels from the top-left of the current origin
//! (the window, or the wizard's content panel).

use std::sync::Arc;

use egui::{
    pos2, vec2, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Galley, Id, Pos2, Rect,
    Sense, Shape, Stroke, StrokeKind, TextureHandle, Ui,
};

use super::assets::Assets;
use super::theme::{self, rgba};

/// The three button image sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Btn {
    /// 282x50 `button_*.png`
    Big,
    /// 212x38 `button_*_middle.png`
    Middle,
    /// 141x25 `button_*_small.png`
    Small,
}

impl Btn {
    pub fn size(self) -> (f32, f32) {
        match self {
            Btn::Big => (282.0, 50.0),
            Btn::Middle => (212.0, 38.0),
            Btn::Small => (141.0, 25.0),
        }
    }

    pub fn w(self) -> f32 {
        self.size().0
    }

    fn images(self) -> [&'static str; 3] {
        match self {
            Btn::Big => ["button_up.png", "button_highlighted.png", "button_down.png"],
            Btn::Middle => [
                "button_up_middle.png",
                "button_highlighted_middle.png",
                "button_down_middle.png",
            ],
            Btn::Small => [
                "button_up_small.png",
                "button_highlighted_small.png",
                "button_down_small.png",
            ],
        }
    }
}

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

pub struct Clicked {
    pub clicked: bool,
    pub hovered: bool,
}

pub struct FieldResponse {
    /// Enter pressed or focus lost after editing -- Swing's action/focusLost commit.
    pub committed: bool,
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

    /// Runs `f` with the origin moved by (`dx`, `dy`).
    pub fn at<R>(&mut self, dx: f32, dy: f32, f: impl FnOnce(&mut Kit) -> R) -> R {
        let saved = self.origin;
        self.origin += vec2(dx, dy);
        let r = f(self);
        self.origin = saved;
        r
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

    /// The translucent magenta section box used everywhere.
    pub fn section_box(&self, x: f32, y: f32, w: f32, h: f32, arc: f32, fill: Color32) {
        self.round_box(x, y, w, h, arc, fill, Some(theme::BOX_BORDER));
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

    /// Bold faked by a double draw (egui has no synthetic bold; Swing derived one).
    pub fn text_left_bold(&self, x: f32, y: f32, h: f32, text: &str, font: FontId, color: Color32) {
        self.text_left(x, y, h, text, font.clone(), color);
        self.text_left(x + 0.6, y, h, text, font, color);
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

    /// `clipboardIcon`
    pub fn clipboard_icon(&self, x: f32, y: f32, size: f32, color: Color32) {
        let o = self.origin + vec2(x, y);
        let s = size;
        let stroke = Stroke::new((s / 13.0).max(1.4), color);
        let (bx, by, bw, bh) = (s / 6.0, s / 5.0, s * 2.0 / 3.0, s * 7.0 / 10.0);
        let board = Rect::from_min_size(o + vec2(bx, by), vec2(bw, bh));
        self.painter()
            .rect_stroke(board, 1.5, stroke, StrokeKind::Middle);
        let (cw, ch) = (s / 3.0, (s / 7.0).max(3.0));
        self.painter().rect_filled(
            Rect::from_min_size(o + vec2(s / 2.0 - cw / 2.0, s / 12.0), vec2(cw, ch)),
            1.0,
            color,
        );
        for fy in [1.0 / 3.0, 3.0 / 5.0] {
            self.painter().line_segment(
                [
                    o + vec2(bx + bw / 5.0, by + bh * fy),
                    o + vec2(bx + bw * 4.0 / 5.0, by + bh * fy),
                ],
                stroke,
            );
        }
    }

    // ---- interaction ----

    /// A plain click/hover area.
    pub fn area(&mut self, key: &str, x: f32, y: f32, w: f32, h: f32, tip: &str) -> Clicked {
        if self.blocked {
            return Clicked {
                clicked: false,
                hovered: false,
            };
        }
        let r = self
            .ui
            .interact(self.rect(x, y, w, h), self.wid(key), Sense::click());
        if r.hovered() {
            self.set_tip(tip);
        }
        Clicked {
            clicked: r.clicked(),
            hovered: r.hovered(),
        }
    }

    pub fn hand_area(&mut self, key: &str, x: f32, y: f32, w: f32, h: f32, tip: &str) -> Clicked {
        let c = self.area(key, x, y, w, h, tip);
        if c.hovered {
            self.ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        c
    }

    /// `SpecialButton`: image states up / highlighted / down with a centered Conthrax label.
    /// A disabled button is dimmed and inert (Swing's `setEnabled(false)` did neither).
    #[allow(clippy::too_many_arguments)]
    pub fn button(
        &mut self,
        key: &str,
        kind: Btn,
        text: &str,
        size: f32,
        x: f32,
        y: f32,
        enabled: bool,
        tip: &str,
    ) -> bool {
        let (w, h) = kind.size();
        let rect = self.rect(x, y, w, h);
        let [up, hi, down] = kind.images();
        let (mut hovered, mut pressed, mut clicked) = (false, false, false);
        if !self.blocked {
            let resp = self.ui.interact(
                rect,
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
            if enabled {
                pressed = resp.is_pointer_button_down_on();
                clicked = resp.clicked();
            }
        }
        let img = if pressed {
            down
        } else if hovered && enabled {
            hi
        } else {
            up
        };
        let tex = self.assets.tex(self.ui.ctx(), img, w as u32, h as u32);
        let tint = if enabled {
            Color32::WHITE
        } else {
            Color32::from_gray(140)
        };
        self.paint_tex(&tex, rect, tint);
        let color = if !enabled {
            Color32::from_gray(150)
        } else if hovered {
            theme::BUTTON_TEXT_HOVER
        } else {
            theme::BUTTON_TEXT
        };
        let g = self.galley(text, theme::conthrax(size), color, None);
        let top = rect.center().y - g.size().y / 2.0;
        self.ui
            .painter()
            .with_clip_rect(rect)
            .galley(pos2(rect.center().x, top), g, color);
        clicked
    }

    /// `makeHeader`: the tipbox banner scaled to 450 wide, with centered Conthrax 14 text
    /// wrapped at 400 (the HTML table width), vertically centered in `h`.
    pub fn header(&self, text: &str, x: f32, y: f32, w: f32, h: f32) {
        let ih = (70.0 * w / 802.0).floor();
        self.image("tipbox_top.png", x, y + ((h - ih) / 2.0).floor(), w, ih);
        self.text_center(
            x,
            y,
            w,
            h,
            text,
            theme::conthrax(14.0),
            theme::WHITE,
            Some(400.0),
        );
    }

    /// `SpecialLabel`: filled rect with centered Conthrax text.
    #[allow(clippy::too_many_arguments)]
    pub fn special_label(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        text: &str,
        size: f32,
        bg: Color32,
        fg: Color32,
    ) {
        self.fill(x, y, w, h, bg);
        self.text_center(x, y, w, h, text, theme::conthrax(size), fg, None);
    }

    /// Natural `SpecialLabel` size: text + 10 in each direction.
    pub fn special_label_size(&self, text: &str, size: f32) -> egui::Vec2 {
        self.text_size(text, theme::conthrax(size)) + vec2(10.0, 10.0)
    }

    /// `SpecialTextfield`: rounded translucent box, white Conthrax text, grey placeholder.
    #[allow(clippy::too_many_arguments)]
    pub fn text_field(
        &mut self,
        key: &str,
        text: &mut String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        size: f32,
        placeholder: &str,
        bg: Color32,
        tip: &str,
    ) -> FieldResponse {
        let rect = self.rect(x, y, w, h);
        self.ui.painter().rect_filled(rect, 4.0, bg);
        let edit = egui::TextEdit::singleline(text)
            .id(self.wid(key))
            .font(theme::conthrax(size))
            .text_color(theme::WHITE)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .vertical_align(Align::Center)
            .desired_width(w)
            .interactive(!self.blocked);
        // Swing's EmptyBorder(2, 8, 2, 8).
        let resp = self.ui.put(rect.shrink2(vec2(8.0, 2.0)), edit);
        if resp.hovered() && !self.blocked {
            self.set_tip(tip);
        }
        if text.is_empty() && !placeholder.is_empty() {
            let g = self.ui.ctx().fonts_mut(|f| {
                f.layout_no_wrap(
                    placeholder.to_string(),
                    theme::conthrax(size),
                    theme::PLACEHOLDER,
                )
            });
            let pos = pos2(rect.min.x + 8.0, rect.center().y - g.size().y / 2.0);
            self.ui
                .painter()
                .with_clip_rect(rect)
                .galley(pos, g, theme::PLACEHOLDER);
        }
        let enter = resp.lost_focus() && self.ui.input(|i| i.key_pressed(egui::Key::Enter));
        FieldResponse {
            committed: resp.lost_focus() || enter,
        }
    }

    pub fn request_focus(&self, key: &str) {
        self.ui.memory_mut(|m| m.request_focus(self.wid(key)));
    }

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

    /// `SpecialHyperlink`: underlined white Conthrax, centered on `cx`; returns its height.
    pub fn link(&mut self, key: &str, text: &str, url: &str, size: f32, cx: f32, y: f32) -> f32 {
        let font = theme::conthrax(size);
        let s = self.text_size(text, font.clone());
        let x = cx - (s.x / 2.0).floor();
        self.text_left(x, y, s.y, text, font, theme::WHITE);
        let r = self.rect(x, y, s.x, s.y);
        self.ui.painter().hline(
            r.x_range(),
            r.bottom() - 2.0,
            Stroke::new(1.0, theme::WHITE),
        );
        if self.hand_area(key, x, y, s.x, s.y, "").clicked {
            crate::core::platform::open_url(url);
        }
        s.y
    }
}
