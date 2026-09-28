//! The launcher's design system: colour tokens, flat widgets, icons and backdrops.
//!
//! Dark, calm surfaces over darkened game art; one cyan accent for primary actions and
//! the active state, magenta for highlights. Conthrax is reserved for headings; body text
//! and labels use Liberation Sans. The installer wizards keep their classic bitmap look
//! (`kit.rs`); everything here is additive.

use egui::{
    pos2, vec2, Color32, CornerRadius, CursorIcon, Id, Mesh, Order, Pos2, Rect, Sense, Shape,
    Stroke, StrokeKind,
};

use super::kit::Kit;
use super::theme;

// ---- tokens ----

pub const BG: Color32 = Color32::from_rgb(0x0D, 0x0B, 0x14);
pub const SURFACE: Color32 = Color32::from_rgba_unmultiplied_const(0x16, 0x13, 0x1F, 224);
pub const SURFACE_SOLID: Color32 = Color32::from_rgb(0x16, 0x13, 0x1F);
pub const SURFACE_HI: Color32 = Color32::from_rgb(0x22, 0x1D, 0x30);
pub const SURFACE_LO: Color32 = Color32::from_rgb(0x0F, 0x0D, 0x17);
pub const BORDER: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 20);
pub const BORDER_HI: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 44);
pub const TEXT: Color32 = Color32::from_rgb(0xF2, 0xF0, 0xF7);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xA3, 0x9F, 0xB3);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x6E, 0x6A, 0x7E);
pub const ACCENT: Color32 = Color32::from_rgb(0x3F, 0xD5, 0xE0);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0x06, 0x1A, 0x1D);
pub const ACCENT_2: Color32 = Color32::from_rgb(0xC8, 0x00, 0x96);
pub const OK: Color32 = Color32::from_rgb(0x3D, 0xDC, 0x84);
pub const WARN: Color32 = Color32::from_rgb(0xFF, 0xB5, 0x47);
pub const DANGER: Color32 = Color32::from_rgb(0xFF, 0x5C, 0x7A);

pub const R_CARD: u8 = 10;
pub const R_CONTROL: u8 = 8;
pub const ANIM: f32 = 0.12;

pub fn body(size: f32) -> egui::FontId {
    theme::arial(size)
}

pub fn bold(size: f32) -> egui::FontId {
    theme::arial_bold(size)
}

pub fn display(size: f32) -> egui::FontId {
    theme::conthrax(size)
}

/// `a` → `b` by `t` (0..=1), per channel including alpha.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

pub fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Play,
    Stop,
    Download,
    Mods,
    Globe,
    Gear,
    Folder,
    Refresh,
    Check,
    Warning,
    Headset,
    More,
    ChevronDown,
    Monitor,
    Info,
}

/// What a flat control reports back.
#[derive(Default, Clone, Copy)]
pub struct Resp {
    pub clicked: bool,
    pub hovered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    Primary,
    Danger,
    Secondary,
    Ghost,
}

impl Kit<'_> {
    fn sid(&self, key: &str) -> Id {
        Id::new(("style", key))
    }

    /// A hover/click area with an animated hover amount (0..1) and an egui tooltip.
    pub fn hot(&mut self, key: &str, r: Rect, enabled: bool, tip: &str) -> (Resp, f32, bool) {
        let id = self.sid(key);
        let mut out = Resp::default();
        let mut pressed = false;
        if !self.blocked {
            let sense = if enabled {
                Sense::click()
            } else {
                Sense::hover()
            };
            let mut resp = self.ui.interact(r, id, sense);
            if !tip.is_empty() {
                resp = resp.on_hover_text(tip);
            }
            out.hovered = resp.hovered();
            if enabled {
                out.clicked = resp.clicked();
                pressed = resp.is_pointer_button_down_on();
                if out.hovered {
                    self.ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
            }
        }
        let t =
            self.ui
                .ctx()
                .animate_bool_with_time(id.with("hover"), out.hovered && enabled, ANIM);
        (out, t, pressed)
    }

    // ---- surfaces ----

    pub fn card(&self, x: f32, y: f32, w: f32, h: f32) {
        let r = self.rect(x, y, w, h);
        self.ui.painter().rect_filled(r, R_CARD, SURFACE);
        self.ui
            .painter()
            .rect_stroke(r, R_CARD, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    }

    /// A card with a small upper-case title at its top-left.
    pub fn titled_card(&self, x: f32, y: f32, w: f32, h: f32, title: &str) {
        self.card(x, y, w, h);
        self.caps(x + 20.0, y + 18.0, title, TEXT_MUTED);
    }

    /// Small letter-spaced upper-case label (section headers, card titles).
    pub fn caps(&self, x: f32, y: f32, text: &str, color: Color32) {
        let spaced: String = text
            .to_uppercase()
            .chars()
            .flat_map(|c| [c, '\u{200A}'])
            .collect();
        self.text_left(x, y, 14.0, spaced.trim_end(), bold(11.0), color);
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

    /// Text clipped to `w` with an ellipsis.
    pub fn text_fit(&self, x: f32, y: f32, w: f32, text: &str, font: egui::FontId, color: Color32) {
        let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
        job.wrap = egui::text::TextWrapping::truncate_at_width(w);
        let g = self.ui.ctx().fonts_mut(|f| f.layout_job(job));
        self.ui.painter().galley(self.origin + vec2(x, y), g, color);
    }

    pub fn text_width(&self, text: &str, font: egui::FontId) -> f32 {
        self.ui
            .ctx()
            .fonts_mut(|f| f.layout_no_wrap(text.to_string(), font, TEXT).size().x)
    }

    // ---- buttons ----

    /// A flat button; `icon` is drawn before the label.
    #[allow(clippy::too_many_arguments)]
    pub fn flat_button(
        &mut self,
        key: &str,
        variant: Variant,
        icon: Option<Icon>,
        label: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        enabled: bool,
        tip: &str,
    ) -> Resp {
        let r = self.rect(x, y, w, h);
        let (resp, t, pressed) = self.hot(key, r, enabled, tip);
        let radius = if h >= 48.0 {
            CornerRadius::from(h / 2.0)
        } else {
            CornerRadius::from(R_CONTROL)
        };
        let (fill, fg, border) = match variant {
            Variant::Primary | Variant::Danger => {
                let base = if variant == Variant::Primary {
                    ACCENT
                } else {
                    ACCENT_2
                };
                let fg = if variant == Variant::Primary {
                    ACCENT_TEXT
                } else {
                    TEXT
                };
                let mut f = mix(base, Color32::WHITE, 0.18 * t);
                if pressed {
                    f = mix(base, Color32::BLACK, 0.18);
                }
                (f, fg, None)
            }
            Variant::Secondary => {
                let f = mix(
                    SURFACE_SOLID,
                    SURFACE_HI,
                    t + if pressed { 0.5 } else { 0.0 },
                );
                (f, TEXT, Some(mix(BORDER, BORDER_HI, t)))
            }
            Variant::Ghost => (
                with_alpha(Color32::WHITE, (14.0 * t) as u8),
                mix(TEXT_DIM, TEXT, t),
                None,
            ),
        };
        let (fill, fg) = if enabled {
            (fill, fg)
        } else {
            (mix(SURFACE_SOLID, BG, 0.3), TEXT_MUTED)
        };
        self.ui.painter().rect_filled(r, radius, fill);
        if let Some(b) = border {
            self.ui
                .painter()
                .rect_stroke(r, radius, Stroke::new(1.0, b), StrokeKind::Inside);
        }
        if variant == Variant::Primary && enabled && h >= 48.0 {
            // Soft glow under the hero button.
            let glow = r.expand(4.0 + 3.0 * t);
            self.ui.painter().rect_stroke(
                glow,
                CornerRadius::from(glow.height() / 2.0),
                Stroke::new(6.0, with_alpha(ACCENT, (28.0 + 30.0 * t) as u8)),
                StrokeKind::Inside,
            );
        }
        let font = if h >= 48.0 { display(16.0) } else { bold(14.0) };
        let tw = if label.is_empty() {
            0.0
        } else {
            self.text_width(label, font.clone())
        };
        let isz = if h >= 48.0 { 18.0 } else { 14.0 };
        let gap = if icon.is_some() && !label.is_empty() {
            10.0
        } else {
            0.0
        };
        let total = tw + gap + if icon.is_some() { isz } else { 0.0 };
        let mut cx = r.center().x - total / 2.0;
        if let Some(i) = icon {
            icon_at(
                self.ui.painter(),
                i,
                pos2(cx, r.center().y - isz / 2.0),
                isz,
                fg,
            );
            cx += isz + gap;
        }
        if !label.is_empty() {
            self.ui.painter().text(
                pos2(cx, r.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                font,
                fg,
            );
        }
        resp
    }

    /// A square icon-only button.
    pub fn icon_button(
        &mut self,
        key: &str,
        icon: Icon,
        x: f32,
        y: f32,
        s: f32,
        tip: &str,
    ) -> Resp {
        self.flat_button(
            key,
            Variant::Secondary,
            Some(icon),
            "",
            x,
            y,
            s,
            s,
            true,
            tip,
        )
    }

    /// A left-rail navigation item: icon over a small label; active = cyan bar + tint.
    pub fn nav_item(
        &mut self,
        key: &str,
        icon: Icon,
        label: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        active: bool,
        tip: &str,
    ) -> bool {
        let r = self.rect(x, y, w, h);
        let (resp, t, _) = self.hot(key, r, true, tip);
        let a = self
            .ui
            .ctx()
            .animate_bool_with_time(self.sid(key).with("active"), active, ANIM);
        let fill = with_alpha(Color32::WHITE, (10.0 * t + 12.0 * a) as u8);
        self.ui
            .painter()
            .rect_filled(r.shrink2(vec2(8.0, 2.0)), R_CONTROL, fill);
        if a > 0.01 {
            let bar =
                Rect::from_min_size(pos2(r.min.x, r.center().y - 14.0 * a), vec2(3.0, 28.0 * a));
            self.ui.painter().rect_filled(bar, 2.0, ACCENT);
        }
        let fg = mix(mix(TEXT_MUTED, TEXT_DIM, t), ACCENT, a);
        icon_at(
            self.ui.painter(),
            icon,
            pos2(r.center().x - 11.0, r.min.y + 12.0),
            22.0,
            fg,
        );
        let lc = mix(mix(TEXT_MUTED, TEXT_DIM, t), TEXT, a);
        self.ui.painter().text(
            pos2(r.center().x, r.max.y - 12.0),
            egui::Align2::CENTER_CENTER,
            label,
            bold(11.0),
            lc,
        );
        resp.clicked
    }

    /// Segmented control; returns the newly picked index.
    pub fn segmented(
        &mut self,
        key: &str,
        options: &[&str],
        selected: usize,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    ) -> Option<usize> {
        let r = self.rect(x, y, w, h);
        self.ui.painter().rect_filled(r, R_CONTROL, SURFACE_LO);
        self.ui
            .painter()
            .rect_stroke(r, R_CONTROL, Stroke::new(1.0, BORDER), StrokeKind::Inside);
        let seg = (w - 6.0) / options.len() as f32;
        let sel_x =
            self.ui
                .ctx()
                .animate_value_with_time(self.sid(key).with("sel"), selected as f32, ANIM);
        let pill = Rect::from_min_size(
            pos2(r.min.x + 3.0 + sel_x * seg, r.min.y + 3.0),
            vec2(seg, h - 6.0),
        );
        self.ui
            .painter()
            .rect_filled(pill, R_CONTROL - 2, SURFACE_HI);
        let mut picked = None;
        for (i, o) in options.iter().enumerate() {
            let sr = Rect::from_min_size(
                pos2(r.min.x + 3.0 + i as f32 * seg, r.min.y + 3.0),
                vec2(seg, h - 6.0),
            );
            let (resp, t, _) = self.hot(&format!("{key}-{i}"), sr, true, "");
            let c = if i == selected {
                TEXT
            } else {
                mix(TEXT_MUTED, TEXT_DIM, t)
            };
            self.ui
                .painter()
                .text(sr.center(), egui::Align2::CENTER_CENTER, *o, bold(13.0), c);
            if resp.clicked && i != selected {
                picked = Some(i);
            }
        }
        picked
    }

    // ---- small parts ----

    /// A rounded pill with an optional status dot; returns its width.
    pub fn pill(&self, x: f32, y: f32, text: &str, color: Color32, dot: bool) -> f32 {
        let font = bold(11.0);
        let tw = self.text_width(text, font.clone());
        let w = tw + if dot { 30.0 } else { 20.0 };
        let r = self.rect(x, y, w, 22.0);
        self.ui
            .painter()
            .rect_filled(r, 11.0, with_alpha(color, 36));
        self.ui.painter().rect_stroke(
            r,
            11.0,
            Stroke::new(1.0, with_alpha(color, 90)),
            StrokeKind::Inside,
        );
        let mut tx = r.min.x + 10.0;
        if dot {
            self.ui
                .painter()
                .circle_filled(pos2(tx + 3.0, r.center().y), 3.5, color);
            tx += 11.0;
        }
        self.ui.painter().text(
            pos2(tx, r.center().y),
            egui::Align2::LEFT_CENTER,
            text,
            font,
            mix(color, TEXT, 0.35),
        );
        w
    }

    /// Width a pill will take.
    pub fn pill_width(&self, text: &str, dot: bool) -> f32 {
        self.text_width(text, bold(11.0)) + if dot { 30.0 } else { 20.0 }
    }

    pub fn progress(&self, x: f32, y: f32, w: f32, fraction: Option<f32>, label: &str) {
        let track = self.rect(x, y + 20.0, w, 6.0);
        self.ui.painter().rect_filled(track, 3.0, SURFACE_HI);
        match fraction {
            Some(f) => {
                let fill = Rect::from_min_size(track.min, vec2(w * f.clamp(0.0, 1.0), 6.0));
                self.ui.painter().rect_filled(fill, 3.0, ACCENT);
            }
            None => {
                // Indeterminate: a sliding segment.
                let t = self.ui.input(|i| i.time) as f32;
                let seg = w * 0.25;
                let pos = ((t * 0.8).fract() * (w + seg)) - seg;
                let a = (track.min.x + pos).max(track.min.x);
                let b = (track.min.x + pos + seg).min(track.max.x);
                if b > a {
                    self.ui.painter().rect_filled(
                        Rect::from_x_y_ranges(a..=b, track.y_range()),
                        3.0,
                        ACCENT,
                    );
                }
                self.ui.ctx().request_repaint();
            }
        }
        self.text_fit(x, y, w, label, body(12.0), TEXT_DIM);
    }

    /// A switch toggle with a label to the right; returns true when flipped.
    #[allow(clippy::too_many_arguments)]
    pub fn toggle(
        &mut self,
        key: &str,
        on: &mut bool,
        label: &str,
        x: f32,
        y: f32,
        enabled: bool,
        tip: &str,
    ) -> bool {
        let lw = self.text_width(label, body(14.0));
        let r = self.rect(x, y, 40.0 + 12.0 + lw, 24.0);
        let (resp, t, _) = self.hot(key, r, enabled, tip);
        if resp.clicked {
            *on = !*on;
        }
        let k = self
            .ui
            .ctx()
            .animate_bool_with_time(self.sid(key).with("on"), *on, ANIM);
        let track = self.rect(x, y + 2.0, 40.0, 20.0);
        let off_c = mix(SURFACE_HI, with_alpha(Color32::WHITE, 40), t * 0.5);
        let c = if enabled {
            mix(off_c, ACCENT, k)
        } else {
            SURFACE_HI
        };
        self.ui.painter().rect_filled(track, 10.0, c);
        let knob = pos2(track.min.x + 10.0 + 20.0 * k, track.center().y);
        self.ui
            .painter()
            .circle_filled(knob, 7.0, if enabled { TEXT } else { TEXT_MUTED });
        let lc = if enabled { TEXT } else { TEXT_MUTED };
        self.ui.painter().text(
            pos2(x + 52.0, y + 12.0) + self.origin.to_vec2(),
            egui::Align2::LEFT_CENTER,
            label,
            body(14.0),
            lc,
        );
        resp.clicked
    }

    /// Flat single-line input with an accent focus ring and placeholder.
    #[allow(clippy::too_many_arguments)]
    pub fn input(
        &mut self,
        key: &str,
        text: &mut String,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        placeholder: &str,
        invalid: bool,
        tip: &str,
    ) -> bool {
        let r = self.rect(x, y, w, h);
        let id = self.sid(key);
        let focused = self.ui.memory(|m| m.has_focus(id));
        self.ui.painter().rect_filled(r, R_CONTROL, SURFACE_LO);
        let border = if invalid {
            DANGER
        } else if focused {
            ACCENT
        } else {
            BORDER_HI
        };
        self.ui
            .painter()
            .rect_stroke(r, R_CONTROL, Stroke::new(1.0, border), StrokeKind::Inside);
        let edit = egui::TextEdit::singleline(text)
            .id(id)
            .font(body(14.0))
            .text_color(TEXT)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .vertical_align(egui::Align::Center)
            .desired_width(w - 24.0)
            .interactive(!self.blocked);
        let resp = self.ui.put(r.shrink2(vec2(12.0, 2.0)), edit);
        let resp = if tip.is_empty() || self.blocked {
            resp
        } else {
            resp.on_hover_text(tip)
        };
        if text.is_empty() && !placeholder.is_empty() {
            self.ui.painter().with_clip_rect(r).text(
                pos2(r.min.x + 12.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                placeholder,
                body(14.0),
                TEXT_MUTED,
            );
        }
        resp.lost_focus()
    }

    // ---- dropdowns / popovers ----

    /// A dropdown showing `options[selected]`; returns a newly picked index.
    #[allow(clippy::too_many_arguments)]
    pub fn dropdown(
        &mut self,
        key: &str,
        options: &[String],
        selected: usize,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tip: &str,
    ) -> Option<usize> {
        let label = options.get(selected).cloned().unwrap_or_default();
        let r = self.rect(x, y, w, h);
        let (resp, t, _) = self.hot(key, r, !options.is_empty(), tip);
        self.ui
            .painter()
            .rect_filled(r, R_CONTROL, mix(SURFACE, SURFACE_HI, t));
        self.ui.painter().rect_stroke(
            r,
            R_CONTROL,
            Stroke::new(1.0, mix(BORDER_HI, with_alpha(ACCENT, 120), t * 0.6)),
            StrokeKind::Inside,
        );
        self.text_fit(
            x + 14.0,
            y + (h - 17.0) / 2.0,
            w - 44.0,
            &label,
            body(14.0),
            TEXT,
        );
        icon_at(
            self.ui.painter(),
            Icon::ChevronDown,
            pos2(r.max.x - 26.0, r.center().y - 6.0),
            12.0,
            TEXT_DIM,
        );
        let open_id = self.sid(key).with("open");
        if resp.clicked {
            let open = self
                .ui
                .ctx()
                .data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
            self.ui.ctx().data_mut(|d| d.insert_temp(open_id, !open));
            return None;
        }
        let entries: Vec<(String, bool)> = options
            .iter()
            .enumerate()
            .map(|(i, o)| (o.clone(), i == selected))
            .collect();
        self.menu_popup(key, r, w, &entries)
            .filter(|i| *i != selected)
    }

    /// A `⋯` button with a menu of actions; returns the picked index.
    pub fn menu_button(
        &mut self,
        key: &str,
        items: &[&str],
        x: f32,
        y: f32,
        s: f32,
        tip: &str,
    ) -> Option<usize> {
        let resp = self.icon_button(key, Icon::More, x, y, s, tip);
        let open_id = self.sid(key).with("open");
        if resp.clicked {
            let open = self
                .ui
                .ctx()
                .data(|d| d.get_temp::<bool>(open_id).unwrap_or(false));
            self.ui.ctx().data_mut(|d| d.insert_temp(open_id, !open));
            return None;
        }
        let r = self.rect(x, y, s, s);
        let entries: Vec<(String, bool)> = items.iter().map(|s| (s.to_string(), false)).collect();
        let w = 200.0;
        let anchor = Rect::from_min_size(pos2(r.max.x - w, r.min.y), vec2(w, s));
        self.menu_popup(key, anchor, w, &entries)
    }

    /// The popup list under `anchor` if `key` is open. Closes on pick or outside click.
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
        let row = 34.0;
        let h = entries.len() as f32 * row + 8.0;
        let screen = self.ui.ctx().content_rect();
        let below = anchor.max.y + 6.0 + h <= screen.max.y;
        let pos = if below {
            pos2(anchor.min.x, anchor.max.y + 6.0)
        } else {
            pos2(anchor.min.x, anchor.min.y - 6.0 - h)
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
                p.rect_filled(
                    rect.translate(vec2(0.0, 4.0)).expand(2.0),
                    R_CARD,
                    with_alpha(Color32::BLACK, 90),
                );
                p.rect_filled(rect, R_CARD, SURFACE_SOLID);
                p.rect_stroke(
                    rect,
                    R_CARD,
                    Stroke::new(1.0, BORDER_HI),
                    StrokeKind::Inside,
                );
                for (i, (label, current)) in entries.iter().enumerate() {
                    let rr = Rect::from_min_size(
                        pos2(rect.min.x + 4.0, rect.min.y + 4.0 + i as f32 * row),
                        vec2(w - 8.0, row),
                    );
                    let resp = ui.interact(rr, open_id.with(i), Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(rr, R_CONTROL - 2, SURFACE_HI);
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    let c = if *current { ACCENT } else { TEXT };
                    let mut job =
                        egui::text::LayoutJob::simple_singleline(label.clone(), body(14.0), c);
                    job.wrap = egui::text::TextWrapping::truncate_at_width(w - 44.0);
                    let g = ui.ctx().fonts_mut(|f| f.layout_job(job));
                    ui.painter().galley(
                        pos2(rr.min.x + 12.0, rr.center().y - g.size().y / 2.0),
                        g,
                        c,
                    );
                    if *current {
                        icon_at(
                            ui.painter(),
                            Icon::Check,
                            pos2(rr.max.x - 26.0, rr.center().y - 7.0),
                            14.0,
                            ACCENT,
                        );
                    }
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

    // ---- backdrop ----

    /// Game art covering the rect, with a left-to-right darkening and a bottom fade.
    pub fn hero_backdrop(&self, name: &str, x: f32, y: f32, w: f32, h: f32) {
        let tex = self.assets.hero(self.ui.ctx(), name, w as u32, h as u32);
        let r = self.rect(x, y, w, h);
        self.ui.painter().image(
            tex.id(),
            r,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        // Overall dim.
        self.ui.painter().rect_filled(r, 0.0, with_alpha(BG, 110));
        // Left gradient for the text block: solid behind the text, fading out by 3/4.
        let solid = r.min.x + w * 0.28;
        self.ui.painter().rect_filled(
            Rect::from_min_max(r.min, pos2(solid, r.max.y)),
            0.0,
            with_alpha(BG, 200),
        );
        gradient(
            self.ui.painter(),
            Rect::from_min_max(pos2(solid, r.min.y), pos2(r.min.x + w * 0.78, r.max.y)),
            [with_alpha(BG, 200), with_alpha(BG, 0)],
            true,
        );
        // Bottom fade for the cards row.
        gradient(
            self.ui.painter(),
            Rect::from_min_max(pos2(r.min.x, r.max.y - h * 0.42), r.max),
            [with_alpha(BG, 0), with_alpha(BG, 235)],
            false,
        );
        // Top fade under the title bar.
        gradient(
            self.ui.painter(),
            Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 90.0)),
            [with_alpha(BG, 170), with_alpha(BG, 0)],
            false,
        );
    }

    /// Soft background for pages without art.
    pub fn plain_backdrop(&self, x: f32, y: f32, w: f32, h: f32) {
        let r = self.rect(x, y, w, h);
        self.ui.painter().rect_filled(r, 0.0, BG);
        gradient(
            self.ui.painter(),
            Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 260.0)),
            [with_alpha(ACCENT_2, 26), with_alpha(ACCENT_2, 0)],
            false,
        );
    }
}

/// A two-colour linear gradient (horizontal when `horizontal`, else vertical).
pub fn gradient(p: &egui::Painter, r: Rect, colors: [Color32; 2], horizontal: bool) {
    let mut mesh = Mesh::default();
    let (a, b) = (colors[0], colors[1]);
    let (tl, tr, bl, br) = if horizontal {
        (a, b, a, b)
    } else {
        (a, a, b, b)
    };
    mesh.colored_vertex(r.left_top(), tl);
    mesh.colored_vertex(r.right_top(), tr);
    mesh.colored_vertex(r.left_bottom(), bl);
    mesh.colored_vertex(r.right_bottom(), br);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    p.add(Shape::mesh(mesh));
}

/// Vector icons in an `s`-sized box at `o`.
pub fn icon_at(p: &egui::Painter, icon: Icon, o: Pos2, s: f32, c: Color32) {
    let st = Stroke::new((s / 11.0).max(1.5), c);
    let pt = |fx: f32, fy: f32| o + vec2(s * fx, s * fy);
    let line = |pts: Vec<Pos2>| {
        p.add(Shape::line(pts, st));
    };
    match icon {
        Icon::Play => {
            p.add(Shape::convex_polygon(
                vec![pt(0.22, 0.12), pt(0.88, 0.5), pt(0.22, 0.88)],
                c,
                Stroke::NONE,
            ));
        }
        Icon::Stop => {
            p.rect_filled(Rect::from_min_max(pt(0.2, 0.2), pt(0.8, 0.8)), 2.0, c);
        }
        Icon::Download => {
            line(vec![pt(0.5, 0.1), pt(0.5, 0.64)]);
            line(vec![pt(0.26, 0.42), pt(0.5, 0.66), pt(0.74, 0.42)]);
            line(vec![
                pt(0.14, 0.72),
                pt(0.14, 0.88),
                pt(0.86, 0.88),
                pt(0.86, 0.72),
            ]);
        }
        Icon::Mods => {
            for (fx, fy) in [(0.1, 0.1), (0.56, 0.1), (0.1, 0.56), (0.56, 0.56)] {
                let rr = Rect::from_min_size(pt(fx, fy), vec2(s * 0.34, s * 0.34));
                if (fx, fy) == (0.56, 0.1) {
                    p.rect_filled(rr, 2.0, c);
                } else {
                    p.rect_stroke(rr, 2.0, st, StrokeKind::Inside);
                }
            }
        }
        Icon::Globe => {
            let cen = pt(0.5, 0.5);
            p.circle_stroke(cen, s * 0.4, st);
            line(vec![pt(0.1, 0.5), pt(0.9, 0.5)]);
            p.add(Shape::ellipse_stroke(cen, vec2(s * 0.16, s * 0.4), st));
        }
        Icon::Gear => {
            let cen = pt(0.5, 0.5);
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let d = vec2(a.cos(), a.sin());
                p.line_segment(
                    [cen + d * s * 0.3, cen + d * s * 0.46],
                    Stroke::new(s / 7.0, c),
                );
            }
            p.circle_stroke(cen, s * 0.28, st);
            p.circle_stroke(cen, s * 0.1, st);
        }
        Icon::Folder => {
            line(vec![
                pt(0.1, 0.84),
                pt(0.1, 0.18),
                pt(0.4, 0.18),
                pt(0.5, 0.3),
                pt(0.9, 0.3),
                pt(0.9, 0.84),
                pt(0.1, 0.84),
            ]);
        }
        Icon::Refresh => {
            let cen = pt(0.5, 0.5);
            let pts: Vec<Pos2> = (0..=20)
                .map(|k| {
                    let a = -0.3 + k as f32 / 20.0 * 4.9;
                    cen + vec2(a.cos(), a.sin()) * s * 0.36
                })
                .collect();
            let end = *pts.last().expect("points");
            line(pts);
            line(vec![
                end + vec2(-s * 0.2, -s * 0.02),
                end,
                end + vec2(-s * 0.02, s * 0.2),
            ]);
        }
        Icon::Check => line(vec![pt(0.16, 0.52), pt(0.4, 0.76), pt(0.86, 0.24)]),
        Icon::Warning => {
            p.add(Shape::convex_polygon(
                vec![pt(0.5, 0.08), pt(0.95, 0.9), pt(0.05, 0.9)],
                Color32::TRANSPARENT,
                st,
            ));
            line(vec![pt(0.5, 0.36), pt(0.5, 0.62)]);
            p.circle_filled(pt(0.5, 0.76), s / 16.0, c);
        }
        Icon::Headset => {
            let rr = Rect::from_min_max(pt(0.06, 0.3), pt(0.94, 0.76));
            p.rect_stroke(rr, s * 0.14, st, StrokeKind::Middle);
            line(vec![pt(0.38, 0.76), pt(0.5, 0.62), pt(0.62, 0.76)]);
            line(vec![
                pt(0.2, 0.3),
                pt(0.3, 0.12),
                pt(0.7, 0.12),
                pt(0.8, 0.3),
            ]);
        }
        Icon::More => {
            for fx in [0.2, 0.5, 0.8] {
                p.circle_filled(pt(fx, 0.5), s / 11.0 + 0.5, c);
            }
        }
        Icon::ChevronDown => line(vec![pt(0.15, 0.32), pt(0.5, 0.68), pt(0.85, 0.32)]),
        Icon::Monitor => {
            p.rect_stroke(
                Rect::from_min_max(pt(0.06, 0.14), pt(0.94, 0.7)),
                2.0,
                st,
                StrokeKind::Middle,
            );
            line(vec![pt(0.5, 0.7), pt(0.5, 0.86)]);
            line(vec![pt(0.3, 0.86), pt(0.7, 0.86)]);
        }
        Icon::Info => {
            p.circle_stroke(pt(0.5, 0.5), s * 0.42, st);
            line(vec![pt(0.5, 0.44), pt(0.5, 0.74)]);
            p.circle_filled(pt(0.5, 0.29), s / 16.0, c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixing() {
        assert_eq!(mix(Color32::BLACK, Color32::WHITE, 0.0), Color32::BLACK);
        assert_eq!(mix(Color32::BLACK, Color32::WHITE, 1.0), Color32::WHITE);
        assert_eq!(mix(Color32::BLACK, Color32::WHITE, 2.0), Color32::WHITE);
    }
}
