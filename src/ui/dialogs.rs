//! Modal dialogs as real child windows: the themed `ErrorDialog`, `JOptionPane`-style
//! message/confirm/option boxes, and the device picker.
//!
//! Screens queue a dialog under a string key and later `take` the answer. While any dialog
//! is open, the owning window stops reacting (`is_open`), like a Swing modal.

use egui::{
    vec2, Color32, Pos2, Rect, Stroke, StrokeKind, ViewportBuilder, ViewportCommand, ViewportId,
};

use super::assets::{self, Assets};
use super::kit::{Btn, Kit};
use super::theme::{self, rgba};
use crate::core::adb::devices::Device;
use crate::core::error::{HelpLink, UiError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    Info,
    Warning,
    Question,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Index of the pressed button (0 = Yes / first option), or the picked device.
    Button(usize),
    /// Closed with the window's X.
    Closed,
}

impl Answer {
    pub fn is_yes(&self) -> bool {
        *self == Answer::Button(0)
    }
}

enum Kind {
    Error(HelpLink),
    Message(Icon, Vec<String>),
    Picker(Vec<Device>),
}

struct Dialog {
    id: u64,
    key: &'static str,
    title: String,
    message: String,
    kind: Kind,
    focused: bool,
}

#[derive(Default)]
pub struct DialogHost {
    stack: Vec<Dialog>,
    answers: Vec<(&'static str, Answer)>,
    next_id: u64,
}

impl DialogHost {
    pub fn is_open(&self) -> bool {
        !self.stack.is_empty()
    }

    fn push(&mut self, key: &'static str, title: &str, message: &str, kind: Kind) {
        self.next_id += 1;
        self.stack.push(Dialog {
            id: self.next_id,
            key,
            title: title.into(),
            message: message.into(),
            kind,
            focused: false,
        });
    }

    /// The themed Marcelus `ErrorDialog` with an optional help link.
    pub fn error(&mut self, title: &str, message: &str, link: HelpLink) {
        self.push("", title, message, Kind::Error(link));
    }

    pub fn error_ui(&mut self, e: &UiError) {
        self.error(&e.title, &e.message, e.link);
    }

    pub fn message(&mut self, key: &'static str, title: &str, message: &str, icon: Icon) {
        self.push(key, title, message, Kind::Message(icon, vec!["OK".into()]));
    }

    pub fn info(&mut self, title: &str, message: &str) {
        self.message("", title, message, Icon::Info);
    }

    pub fn warning(&mut self, title: &str, message: &str) {
        self.message("", title, message, Icon::Warning);
    }

    /// Yes/No; `Answer::Button(0)` is Yes.
    pub fn confirm(&mut self, key: &'static str, title: &str, message: &str, icon: Icon) {
        self.push(
            key,
            title,
            message,
            Kind::Message(icon, vec!["Yes".into(), "No".into()]),
        );
    }

    pub fn options(
        &mut self,
        key: &'static str,
        title: &str,
        message: &str,
        icon: Icon,
        buttons: &[&str],
    ) {
        self.push(
            key,
            title,
            message,
            Kind::Message(icon, buttons.iter().map(|b| b.to_string()).collect()),
        );
    }

    /// "Which one is your Quest?" -- `Answer::Button(i)` is `devices[i]`.
    pub fn device_picker(&mut self, key: &'static str, devices: Vec<Device>) {
        self.push(key, "Which one is your Quest?", "", Kind::Picker(devices));
    }

    /// Answers waiting to be taken; the owner should run another frame.
    pub fn has_answers(&self) -> bool {
        !self.answers.is_empty()
    }

    pub fn take(&mut self, key: &'static str) -> Option<Answer> {
        let i = self.answers.iter().position(|(k, _)| *k == key)?;
        Some(self.answers.remove(i).1)
    }

    /// Draws the top-most dialog as a child window of the current viewport.
    pub fn show(&mut self, ctx: &egui::Context, assets: &Assets, parent: Option<Rect>) {
        let Some(top) = self.stack.last_mut() else {
            return;
        };
        let size = dialog_size(ctx, top);
        let mut builder = ViewportBuilder::default()
            .with_title(top.title.clone())
            .with_inner_size(size)
            .with_resizable(false)
            .with_minimize_button(false)
            .with_maximize_button(false)
            .with_icon(std::sync::Arc::new(assets::icon()));
        if let Some(p) = parent {
            builder = builder.with_position(Pos2::new(
                p.center().x - size.x / 2.0,
                p.center().y - size.y / 2.0,
            ));
        }
        let vid = ViewportId::from_hash_of(("dialog", top.id));
        let answer = ctx.show_viewport_immediate(vid, builder, |ui, _class| {
            if !top.focused {
                top.focused = true;
                ui.ctx().send_viewport_cmd(ViewportCommand::Focus);
            }
            if ui.input(|i| i.viewport().close_requested()) {
                return Some(Answer::Closed);
            }
            let mut kit = Kit::new(ui, assets, ("dialog", top.id), false);
            let answer = match &top.kind {
                Kind::Error(link) => draw_error(&mut kit, size, &top.message, *link),
                Kind::Message(icon, buttons) => {
                    draw_message(&mut kit, size, &top.message, *icon, buttons)
                }
                Kind::Picker(devices) => draw_picker(&mut kit, size, devices),
            };
            let enter = matches!(top.kind, Kind::Message(..) | Kind::Error(_))
                && ui.input(|i| i.key_pressed(egui::Key::Enter));
            answer.or(enter.then_some(Answer::Button(0)))
        });
        if let Some(a) = answer {
            let d = self.stack.pop().expect("top exists");
            if !d.key.is_empty() {
                self.answers.push((d.key, a));
            }
        }
    }
}

fn message_lines(msg: &str) -> String {
    msg.trim_end().to_string()
}

fn dialog_size(ctx: &egui::Context, d: &Dialog) -> egui::Vec2 {
    let measure = |text: &str, font| {
        ctx.fonts_mut(|f| {
            f.layout_no_wrap(text.to_string(), font, Color32::WHITE)
                .size()
        })
    };
    match &d.kind {
        Kind::Error(link) => {
            let s = measure(&message_lines(&d.message), theme::conthrax(14.0)) + vec2(10.0, 10.0);
            let link_h = if *link == HelpLink::None { 0.0 } else { 18.0 };
            let h = 30.0 + s.y + 12.0 + link_h + 6.0 + 25.0 + 20.0;
            vec2(800.0f32.max(s.x + 40.0), h.max(200.0))
        }
        Kind::Message(_, buttons) => {
            let s = measure(&message_lines(&d.message), theme::arial_bold(12.0));
            let bw: f32 = buttons.iter().map(|b| button_w(ctx, b) + 6.0).sum();
            let w = (s.x + 90.0).max(bw + 40.0).max(260.0);
            let h = s.y.max(32.0) + 30.0 + 26.0 + 24.0;
            vec2(w.ceil(), h.ceil())
        }
        Kind::Picker(devices) => vec2(500.0, 96.0 + devices.len().min(5) as f32 * 70.0 + 40.0),
    }
}

fn button_w(ctx: &egui::Context, text: &str) -> f32 {
    let tw = ctx.fonts_mut(|f| {
        f.layout_no_wrap(text.to_string(), theme::arial_bold(12.0), Color32::BLACK)
            .size()
            .x
    });
    (tw + 32.0).max(72.0)
}

/// `ErrorDialog`: Marcelus banner, message in a `SpecialLabel`, optional link, Close.
fn draw_error(kit: &mut Kit, size: egui::Vec2, message: &str, link: HelpLink) -> Option<Answer> {
    kit.fill(0.0, 0.0, size.x, size.y, Color32::from_rgb(20, 20, 30));
    kit.image("Marcelus.png", 0.0, 0.0, 900.0, 200.0);
    let msg = message_lines(message);
    let s = kit.special_label_size(&msg, 14.0);
    let x = ((size.x - s.x) / 2.0).floor();
    kit.special_label(x, 30.0, s.x, s.y, &msg, 14.0, theme::LABEL_BG, theme::WHITE);
    let mut next_y = 30.0 + s.y + 12.0;
    let dev_mode = "https://learn.adafruit.com/sideloading-on-oculus-quest/enable-developer-mode";
    match link {
        HelpLink::DeveloperMode => {
            next_y += kit.link(
                "help",
                "How to enable Developer Mode on your Quest",
                dev_mode,
                14.0,
                size.x / 2.0,
                next_y,
            );
        }
        HelpLink::UsbDebugging => {
            next_y += kit.link(
                "help",
                "How to allow USB debugging on your Quest",
                dev_mode,
                14.0,
                size.x / 2.0,
                next_y,
            );
        }
        HelpLink::None => {}
    }
    let bx = ((size.x - Btn::Small.w()) / 2.0).floor();
    kit.button(
        "close",
        Btn::Small,
        "Close",
        14.0,
        bx,
        next_y + 6.0,
        true,
        "",
    )
    .then_some(Answer::Button(0))
}

fn draw_icon(kit: &Kit, icon: Icon, x: f32, y: f32) {
    let c = kit.rect(x, y, 32.0, 32.0).center();
    let p = kit.ui.painter();
    match icon {
        Icon::Info | Icon::Question => {
            p.circle_filled(c, 15.0, Color32::from_rgb(88, 130, 196));
            p.circle_stroke(c, 15.0, Stroke::new(1.0, Color32::from_rgb(40, 70, 130)));
            let t = if icon == Icon::Info { "i" } else { "?" };
            p.text(
                c,
                egui::Align2::CENTER_CENTER,
                t,
                theme::arial_bold(20.0),
                Color32::WHITE,
            );
        }
        Icon::Warning => {
            let pts = vec![
                c + vec2(0.0, -15.0),
                c + vec2(16.0, 14.0),
                c + vec2(-16.0, 14.0),
            ];
            p.add(egui::Shape::convex_polygon(
                pts,
                Color32::from_rgb(250, 200, 50),
                Stroke::new(1.0, Color32::from_rgb(150, 110, 0)),
            ));
            p.text(
                c + vec2(0.0, 3.0),
                egui::Align2::CENTER_CENTER,
                "!",
                theme::arial_bold(18.0),
                Color32::BLACK,
            );
        }
    }
}

/// A Metal/Ocean-look `JOptionPane`.
fn draw_message(
    kit: &mut Kit,
    size: egui::Vec2,
    message: &str,
    icon: Icon,
    buttons: &[String],
) -> Option<Answer> {
    kit.fill(0.0, 0.0, size.x, size.y, Color32::from_rgb(238, 238, 238));
    draw_icon(kit, icon, 16.0, 16.0);
    let msg = message_lines(message);
    let font = theme::arial_bold(12.0);
    let text_color = Color32::from_rgb(51, 51, 51);
    let g = kit
        .ui
        .ctx()
        .fonts_mut(|f| f.layout_no_wrap(msg.clone(), font, text_color));
    let text_h = g.size().y.max(32.0);
    kit.ui.painter().galley(
        kit.origin + vec2(64.0, 16.0 + (text_h - g.size().y) / 2.0),
        g,
        text_color,
    );

    let ctx = kit.ctx();
    let widths: Vec<f32> = buttons.iter().map(|b| button_w(&ctx, b)).collect();
    let total: f32 = widths.iter().sum::<f32>() + 6.0 * (buttons.len().saturating_sub(1)) as f32;
    let mut x = ((size.x - total) / 2.0).floor();
    let y = 16.0 + text_h + 18.0;
    let mut answer = None;
    for (i, (b, w)) in buttons.iter().zip(&widths).enumerate() {
        let c = kit.area(&format!("opt{i}"), x, y, *w, 26.0, "");
        let r = kit.rect(x, y, *w, 26.0);
        let p = kit.ui.painter();
        let (top, bottom) = if c.hovered {
            (Color32::WHITE, Color32::from_rgb(200, 221, 242))
        } else {
            (
                Color32::from_rgb(250, 251, 253),
                Color32::from_rgb(221, 232, 243),
            )
        };
        p.rect_filled(
            Rect::from_min_max(r.min, egui::pos2(r.max.x, r.center().y)),
            0.0,
            top,
        );
        p.rect_filled(
            Rect::from_min_max(egui::pos2(r.min.x, r.center().y), r.max),
            0.0,
            bottom,
        );
        let border = if i == 0 {
            rgba(99, 130, 191, 255)
        } else {
            rgba(122, 138, 153, 255)
        };
        p.rect_stroke(r, 0.0, Stroke::new(1.0, border), StrokeKind::Inside);
        p.text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            b,
            theme::arial_bold(12.0),
            Color32::BLACK,
        );
        if c.clicked {
            answer = Some(Answer::Button(i));
        }
        x += w + 6.0;
    }
    answer
}

/// `DevicePickerDialog`.
fn draw_picker(kit: &mut Kit, size: egui::Vec2, devices: &[Device]) -> Option<Answer> {
    kit.image("Echox720.png", 0.0, 0.0, 1280.0, 720.0);
    kit.text_center(
        40.0,
        24.0,
        size.x - 80.0,
        56.0,
        "More than one device is plugged in.\nWhich one is your Quest?",
        theme::arial_bold(12.0),
        theme::WHITE,
        None,
    );
    let x = ((size.x - Btn::Big.w()) / 2.0).floor();
    for (i, d) in devices.iter().take(5).enumerate() {
        if kit.button(
            &format!("dev{i}"),
            Btn::Big,
            &d.label(),
            16.0,
            x,
            96.0 + i as f32 * 70.0,
            true,
            "",
        ) {
            return Some(Answer::Button(i));
        }
    }
    None
}
