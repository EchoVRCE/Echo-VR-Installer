//! The launcher's widgets, drawn in the installer's visual language: slanted bitmap
//! buttons (9-slice stretched to any width), magenta translucent panels with the dark
//! 1px rim, the wine sidebar colour, banner headers with cyan stripes, the blue status
//! bar and the green/grey step chips. Page layout is the launcher's; the look is the
//! installer's (`kit.rs`, `wizard.rs`).

use egui::{
    pos2, vec2, Color32, CursorIcon, Id, Mesh, Order, Pos2, Rect, Sense, Shape, Stroke, StrokeKind,
    TextureId,
};

use super::assets;
use super::kit::Kit;
use super::theme;

// ---- palette (the installer's) ----

/// Base behind the art.
pub const BG: Color32 = Color32::from_rgb(0x0D, 0x0B, 0x14);
/// Section box fill: the installer's magenta, a little stronger over the dark art.
pub const SURFACE: Color32 = Color32::from_rgba_unmultiplied_const(200, 0, 150, 70);
/// Wine sidebar / popup colour.
pub const SURFACE_SOLID: Color32 = Color32::from_rgba_unmultiplied_const(100, 0, 50, 240);
/// Hover rows.
pub const SURFACE_HI: Color32 = Color32::from_rgba_unmultiplied_const(200, 0, 150, 150);
/// Text field background (`SpecialTextfield`).
pub const SURFACE_LO: Color32 = Color32::from_rgba_unmultiplied_const(30, 30, 30, 200);
/// `BOX_BORDER`
pub const BORDER: Color32 = theme::BOX_BORDER;
pub const BORDER_HI: Color32 = Color32::from_rgba_unmultiplied_const(50, 50, 50, 230);
pub const TEXT: Color32 = Color32::WHITE;
pub const TEXT_DIM: Color32 = Color32::from_rgb(225, 215, 228);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(192, 180, 198);
/// The cyan of the banner stripes.
pub const ACCENT: Color32 = Color32::from_rgb(63, 193, 201);
pub const ACCENT_2: Color32 = Color32::from_rgb(200, 0, 150);
/// Step-chip green.
pub const OK: Color32 = theme::CHIP_CURRENT_BG;
pub const CHIP_OFF: Color32 = theme::CHIP_UPCOMING_BG;
pub const WARN: Color32 = Color32::from_rgb(210, 140, 20);
pub const DANGER: Color32 = theme::MARK_BAD;

pub const R_CARD: u8 = 8; // arc 15
pub const R_CONTROL: u8 = 4; // arc 8
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

/// The installer's status-bar pulse (`phase += 0.15` per 50 ms), 0..1.
pub fn pulse(ctx: &egui::Context) -> f32 {
    let t = ctx.input(|i| i.time) as f32;
    (t * 3.0).sin() * 0.5 + 0.5
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

/// What a control reports back.
#[derive(Default, Clone, Copy)]
pub struct Resp {
    pub clicked: bool,
    pub hovered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// The big slanted button (PLAY, INSTALL).
    Primary,
    /// Primary, tinted magenta (STOP).
    Danger,
    /// A regular slanted button.
    Secondary,
    /// A regular slanted button, slightly transparent (minor actions).
    Ghost,
}

// ---- 9-slice ----

/// Draws `tex` into `r`, keeping the left/right caps (in texture pixels) unstretched
/// relative to the height and stretching only the middle horizontally.
pub fn nine_h(
    p: &egui::Painter,
    tex: TextureId,
    tex_size: (f32, f32),
    r: Rect,
    caps: (f32, f32),
    tint: Color32,
) {
    let (tw, th) = tex_size;
    let s = r.height() / th;
    let (mut l, mut rr) = (caps.0 * s, caps.1 * s);
    if l + rr > r.width() {
        let k = r.width() / (l + rr);
        l *= k;
        rr *= k;
    }
    let (ul, ur) = (caps.0 / tw, 1.0 - caps.1 / tw);
    let parts = [
        (r.min.x, r.min.x + l, 0.0, ul),
        (r.min.x + l, r.max.x - rr, ul, ur),
        (r.max.x - rr, r.max.x, ur, 1.0),
    ];
    for (x0, x1, u0, u1) in parts {
        if x1 > x0 {
            p.image(
                tex,
                Rect::from_min_max(pos2(x0, r.min.y), pos2(x1, r.max.y)),
                Rect::from_min_max(pos2(u0, 0.0), pos2(u1, 1.0)),
                tint,
            );
        }
    }
}

/// Button image set for a target height, with its cut-corner cap width (px in the image).
fn button_set(h: f32) -> ([&'static str; 3], f32) {
    if h < 32.0 {
        (
            [
                "button_up_small.png",
                "button_highlighted_small.png",
                "button_down_small.png",
            ],
            9.0,
        )
    } else if h < 44.0 {
        (
            [
                "button_up_middle.png",
                "button_highlighted_middle.png",
                "button_down_middle.png",
            ],
            13.0,
        )
    } else {
        (
            ["button_up.png", "button_highlighted.png", "button_down.png"],
            16.0,
        )
    }
}

impl Kit<'_> {
    fn sid(&self, key: &str) -> Id {
        Id::new(("style", key))
    }

    fn native_tex(&self, name: &str) -> (TextureId, (f32, f32)) {
        let (w, h) = assets::native_size(name);
        let t = self.assets.tex(self.ui.ctx(), name, w, h);
        (t.id(), (w as f32, h as f32))
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

    /// The installer's section box: translucent magenta, dark 1px rim, arc 15.
    pub fn card(&self, x: f32, y: f32, w: f32, h: f32) {
        let r = self.rect(x, y, w, h);
        self.ui.painter().rect_filled(r, R_CARD, SURFACE);
        self.ui
            .painter()
            .rect_stroke(r, R_CARD, Stroke::new(1.0, BORDER), StrokeKind::Inside);
    }

    /// The `tipbox_top` banner (cyan-striped caps, dark slanted middle) with Conthrax text.
    pub fn banner(&self, x: f32, y: f32, w: f32, h: f32, text: &str, size: f32) {
        let (tex, sz) = self.native_tex("tipbox_top.png");
        nine_h(
            self.ui.painter(),
            tex,
            sz,
            self.rect(x, y, w, h),
            (150.0, 170.0),
            Color32::WHITE,
        );
        self.text_center(x, y, w, h, text, display(size), TEXT, None);
    }

    /// Banner width that fits `text` at `h` (caps plus the text).
    pub fn banner_width(&self, text: &str, h: f32, size: f32) -> f32 {
        self.text_width(text, display(size)) + h * 4.4
    }

    /// A section box with a banner title strip at its top-left.
    pub fn titled_card(&self, x: f32, y: f32, w: f32, h: f32, title: &str) {
        self.card(x, y, w, h);
        let bw = self.banner_width(title, 26.0, 12.0).min(w - 24.0);
        self.banner(x + 12.0, y + 10.0, bw, 26.0, title, 12.0);
    }

    /// Section label: Conthrax, like the installer's small headings.
    pub fn caps(&self, x: f32, y: f32, text: &str, color: Color32) {
        self.text_left(x, y, 14.0, text, display(11.0), color);
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

    /// The installer's slanted bitmap button, stretched to `w`x`h`, with an optional icon.
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
        let ([up, hi, down], cap) = button_set(h);
        let img = if pressed {
            down
        } else if t > 0.5 {
            hi
        } else {
            up
        };
        let (tex, sz) = self.native_tex(img);
        let tint = match (enabled, variant) {
            (false, _) => Color32::from_gray(140),
            (true, Variant::Danger) => Color32::from_rgb(255, 150, 215),
            (true, Variant::Ghost) => Color32::from_rgba_unmultiplied(255, 255, 255, 215),
            _ => Color32::WHITE,
        };
        nine_h(self.ui.painter(), tex, sz, r, (cap, cap), tint);
        let fg = if !enabled {
            Color32::from_gray(150)
        } else {
            mix(theme::BUTTON_TEXT, theme::BUTTON_TEXT_HOVER, t)
        };
        let size = if h >= 48.0 {
            22.0
        } else if h >= 36.0 {
            13.0
        } else {
            11.0
        };
        let font = display(size);
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
            self.ui.painter().with_clip_rect(r).text(
                pos2(cx, r.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                font,
                fg,
            );
        }
        resp
    }

    /// A square slanted button with just an icon.
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

    /// A rail item: icon over a small label. Active = green step chip, hover = magenta.
    #[allow(clippy::too_many_arguments)]
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
        let inner = r.shrink2(vec2(8.0, 2.0));
        let fill = mix(
            with_alpha(ACCENT_2, (110.0 * t) as u8),
            theme::CHIP_CURRENT_BG,
            a,
        );
        if t > 0.01 || a > 0.01 {
            self.ui.painter().rect_filled(inner, R_CONTROL, fill);
        }
        if a > 0.01 {
            self.ui.painter().rect_stroke(
                inner,
                R_CONTROL,
                Stroke::new(1.0, BORDER),
                StrokeKind::Inside,
            );
        }
        let fg = mix(TEXT_MUTED, TEXT, t.max(a));
        icon_at(
            self.ui.painter(),
            icon,
            pos2(r.center().x - 11.0, r.min.y + 12.0),
            22.0,
            fg,
        );
        self.ui.painter().text(
            pos2(r.center().x, r.max.y - 12.0),
            egui::Align2::CENTER_CENTER,
            label,
            display(8.5),
            fg,
        );
        resp.clicked
    }

    /// A row of step chips; returns the newly picked index.
    #[allow(clippy::too_many_arguments)]
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
        let gap = 8.0;
        let cw = (w - gap * (options.len() as f32 - 1.0)) / options.len() as f32;
        let mut picked = None;
        for (i, o) in options.iter().enumerate() {
            let cx = x + i as f32 * (cw + gap);
            let r = self.rect(cx, y, cw, h);
            let (resp, t, _) = self.hot(&format!("{key}-{i}"), r, i != selected, "");
            let bg = if i == selected {
                theme::CHIP_CURRENT_BG
            } else {
                mix(theme::CHIP_UPCOMING_BG, theme::CHIP_DONE_BG, t)
            };
            self.ui.painter().rect_filled(r, R_CONTROL, bg);
            self.ui.painter().rect_stroke(
                r,
                R_CONTROL,
                Stroke::new(1.0, BORDER),
                StrokeKind::Inside,
            );
            self.ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                *o,
                display(11.0),
                TEXT,
            );
            if resp.clicked {
                picked = Some(i);
            }
        }
        picked
    }

    // ---- small parts ----

    /// A step-chip badge in `color` (green = on, grey = neutral); returns its width.
    pub fn pill(&self, x: f32, y: f32, text: &str, color: Color32, dot: bool) -> f32 {
        let font = display(9.5);
        let tw = self.text_width(text, font.clone());
        let w = tw + if dot { 28.0 } else { 18.0 };
        let r = self.rect(x, y, w, 22.0);
        self.ui.painter().rect_filled(r, R_CONTROL, color);
        self.ui
            .painter()
            .rect_stroke(r, R_CONTROL, Stroke::new(1.0, BORDER), StrokeKind::Inside);
        let mut tx = r.min.x + 9.0;
        if dot {
            self.ui
                .painter()
                .circle_filled(pos2(tx + 3.0, r.center().y), 3.5, TEXT);
            tx += 10.0;
        }
        self.ui.painter().text(
            pos2(tx, r.center().y),
            egui::Align2::LEFT_CENTER,
            text,
            font,
            TEXT,
        );
        w
    }

    pub fn pill_width(&self, text: &str, dot: bool) -> f32 {
        self.text_width(text, display(9.5)) + if dot { 28.0 } else { 18.0 }
    }

    /// The installer's progress label (white box, black Conthrax) with a green fill.
    pub fn progress(&self, x: f32, y: f32, w: f32, fraction: Option<f32>, label: &str) {
        let r = self.rect(x, y + 4.0, w, 26.0);
        self.ui.painter().rect_filled(r, 0.0, theme::PROGRESS_BG);
        match fraction {
            Some(f) => {
                let fill = Rect::from_min_size(r.min, vec2(w * f.clamp(0.0, 1.0), r.height()));
                self.ui
                    .painter()
                    .rect_filled(fill, 0.0, with_alpha(theme::STATUS_DONE, 200));
            }
            None => {
                // Indeterminate: a sliding green segment.
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
        self.ui
            .painter()
            .rect_stroke(r, 0.0, Stroke::new(1.0, BORDER), StrokeKind::Inside);
        let mut job = egui::text::LayoutJob::simple_singleline(
            label.to_string(),
            display(11.0),
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

    /// The installer's Metal checkbox with a Conthrax label; returns true when flipped.
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
        let w = self.text_width(label, display(12.0)) + 24.0;
        self.checkbox(key, on, label, 12.0, x, y, w, 24.0, false, enabled, tip)
    }

    /// `SpecialTextfield`: dark translucent box, white Conthrax text, magenta focus rim.
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
        let bg = if invalid {
            theme::FIELD_BG_INVALID
        } else {
            SURFACE_LO
        };
        self.ui.painter().rect_filled(r, R_CONTROL, bg);
        let rim = if focused { ACCENT_2 } else { BORDER };
        self.ui
            .painter()
            .rect_stroke(r, R_CONTROL, Stroke::new(1.0, rim), StrokeKind::Inside);
        let edit = egui::TextEdit::singleline(text)
            .id(id)
            .font(display(12.0))
            .text_color(TEXT)
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .vertical_align(egui::Align::Center)
            .desired_width(w - 20.0)
            .interactive(!self.blocked);
        let resp = self.ui.put(r.shrink2(vec2(10.0, 2.0)), edit);
        let resp = if tip.is_empty() || self.blocked {
            resp
        } else {
            resp.on_hover_text(tip)
        };
        if text.is_empty() && !placeholder.is_empty() {
            self.ui.painter().with_clip_rect(r).text(
                pos2(r.min.x + 10.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                placeholder,
                display(12.0),
                theme::PLACEHOLDER,
            );
        }
        resp.lost_focus()
    }

    // ---- dropdowns / popovers ----

    /// A slanted button showing `options[selected]` with a chevron; returns a new pick.
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
        let (resp, t, pressed) = self.hot(key, r, !options.is_empty(), tip);
        let ([up, hi, down], cap) = button_set(h);
        let img = if pressed {
            down
        } else if t > 0.5 {
            hi
        } else {
            up
        };
        let (tex, sz) = self.native_tex(img);
        nine_h(self.ui.painter(), tex, sz, r, (cap, cap), Color32::WHITE);
        let fg = mix(theme::BUTTON_TEXT, theme::BUTTON_TEXT_HOVER, t);
        self.text_fit(
            x + 22.0,
            y + (h - 16.0) / 2.0,
            w - 60.0,
            &label,
            display(13.0),
            fg,
        );
        icon_at(
            self.ui.painter(),
            Icon::ChevronDown,
            pos2(r.max.x - 34.0, r.center().y - 6.0),
            12.0,
            fg,
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
        let w = 210.0;
        let anchor = Rect::from_min_size(pos2(r.max.x - w, r.min.y), vec2(w, s));
        self.menu_popup(key, anchor, w, &entries)
    }

    /// The popup list under `anchor` if `key` is open: a wine panel with magenta hover
    /// rows. Closes on pick, outside click or Escape.
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
                        ui.painter().rect_filled(rr, R_CONTROL, SURFACE_HI);
                        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    }
                    let mut job = egui::text::LayoutJob::simple_singleline(
                        label.clone(),
                        display(12.0),
                        TEXT,
                    );
                    job.wrap = egui::text::TextWrapping::truncate_at_width(w - 48.0);
                    let g = ui.ctx().fonts_mut(|f| f.layout_job(job));
                    ui.painter().galley(
                        pos2(rr.min.x + 12.0, rr.center().y - g.size().y / 2.0),
                        g,
                        TEXT,
                    );
                    if *current {
                        let c = rr.max - vec2(24.0, row / 2.0 + 8.0);
                        icon_at(ui.painter(), Icon::Check, c, 16.0, theme::MARK_OK);
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
        self.ui.painter().rect_filled(r, 0.0, with_alpha(BG, 110));
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
        gradient(
            self.ui.painter(),
            Rect::from_min_max(pos2(r.min.x, r.max.y - h * 0.42), r.max),
            [with_alpha(BG, 0), with_alpha(BG, 235)],
            false,
        );
        gradient(
            self.ui.painter(),
            Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + 90.0)),
            [with_alpha(BG, 170), with_alpha(BG, 0)],
            false,
        );
    }

    /// Pages without a hero: the same Echo art, dimmed further.
    pub fn plain_backdrop(&self, x: f32, y: f32, w: f32, h: f32) {
        let tex = self
            .assets
            .hero(self.ui.ctx(), "hero_quest.jpg", w as u32, h as u32);
        let r = self.rect(x, y, w, h);
        self.ui.painter().image(
            tex.id(),
            r,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        self.ui.painter().rect_filled(r, 0.0, with_alpha(BG, 190));
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

    #[test]
    fn button_sets_by_height() {
        assert_eq!(button_set(25.0).0[0], "button_up_small.png");
        assert_eq!(button_set(38.0).0[0], "button_up_middle.png");
        assert_eq!(button_set(56.0).0[0], "button_up.png");
    }
}
