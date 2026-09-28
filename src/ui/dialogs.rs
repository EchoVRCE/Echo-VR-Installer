//! Modal dialogs as real child windows: errors (with help links), message/confirm/option
//! boxes and the device picker, in the launcher's style.
//!
//! Screens queue a dialog under a string key and later `take` the answer. While any dialog
//! is open, the owning window stops reacting (`is_open`), like a Swing modal.

use egui::{vec2, Pos2, Rect, ViewportBuilder, ViewportCommand, ViewportId};

use super::assets::{self, Assets};
use super::kit::Kit;
use super::style;
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

    /// An error with an optional help link.
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

    pub fn take(&mut self, key: &'static str) -> Option<Answer> {
        let i = self.answers.iter().position(|(k, _)| *k == key)?;
        Some(self.answers.remove(i).1)
    }

    /// Draws the top-most dialog as a child window of the current viewport.
    pub fn show(&mut self, ctx: &egui::Context, assets: &Assets, parent: Option<Rect>) {
        let Some(top) = self.stack.last_mut() else {
            return;
        };
        let size = match &top.kind {
            Kind::Picker(devices) => picker_size(devices),
            _ => modern_size(ctx, top),
        };
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
                Kind::Error(link) => {
                    let buttons = ["Close".to_string()];
                    draw_modern(&mut kit, size, &top.message, Icon::Warning, &buttons, *link)
                }
                Kind::Message(icon, buttons) => {
                    draw_modern(&mut kit, size, &top.message, *icon, buttons, HelpLink::None)
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

fn picker_size(devices: &[Device]) -> egui::Vec2 {
    vec2(460.0, 96.0 + devices.len().min(5) as f32 * 50.0 + 24.0)
}

const MODERN_W: f32 = 460.0;

fn modern_size(ctx: &egui::Context, d: &Dialog) -> egui::Vec2 {
    let text_h = ctx
        .fonts_mut(|f| {
            f.layout(
                message_lines(&d.message),
                style::body(14.0),
                style::TEXT,
                MODERN_W - 104.0,
            )
            .size()
            .y
        })
        .max(40.0);
    let link_h = match &d.kind {
        Kind::Error(l) if *l != HelpLink::None => 30.0,
        _ => 0.0,
    };
    vec2(
        MODERN_W,
        (24.0 + text_h + link_h + 24.0 + 40.0 + 24.0).ceil(),
    )
}

/// The launcher's dark dialog: icon, wrapped text, optional help link, flat buttons.
fn draw_modern(
    kit: &mut Kit,
    size: egui::Vec2,
    message: &str,
    icon: Icon,
    buttons: &[String],
    link: HelpLink,
) -> Option<Answer> {
    kit.fill(0.0, 0.0, size.x, size.y, style::SURFACE_SOLID);
    let (si, color) = match icon {
        Icon::Info => (style::Icon::Info, style::ACCENT),
        Icon::Question => (style::Icon::Info, style::ACCENT),
        Icon::Warning => (style::Icon::Warning, style::WARN),
    };
    let ib = kit.rect(24.0, 24.0, 40.0, 40.0);
    kit.ui
        .painter()
        .rect_filled(ib, style::R_CONTROL, style::with_alpha(color, 36));
    style::icon_at(kit.ui.painter(), si, ib.min + vec2(9.0, 9.0), 22.0, color);
    let mut job = egui::text::LayoutJob::simple(
        message_lines(message),
        style::body(14.0),
        style::TEXT,
        size.x - 104.0,
    );
    job.halign = egui::Align::LEFT;
    let g = kit.ui.ctx().fonts_mut(|f| f.layout_job(job));
    let text_h = g.size().y.max(40.0);
    kit.ui.painter().galley(
        kit.rect(80.0, 24.0 + (40.0 - g.size().y).max(0.0) / 2.0, 0.0, 0.0)
            .min,
        g,
        style::TEXT,
    );
    let mut y = 24.0 + text_h + 24.0;
    let dev_mode = "https://learn.adafruit.com/sideloading-on-oculus-quest/enable-developer-mode";
    let link_text = match link {
        HelpLink::DeveloperMode => Some("How to enable Developer Mode on your Quest"),
        HelpLink::UsbDebugging => Some("How to allow USB debugging on your Quest"),
        HelpLink::None => None,
    };
    if let Some(t) = link_text {
        if kit
            .flat_button(
                "dlg-link",
                style::Variant::Ghost,
                Some(style::Icon::Info),
                t,
                72.0,
                y - 14.0,
                size.x - 96.0,
                30.0,
                true,
                dev_mode,
            )
            .clicked
        {
            crate::core::platform::open_url(dev_mode);
        }
        y += 30.0;
    }
    let bw = 112.0;
    let mut x =
        size.x - 24.0 - bw * buttons.len() as f32 - 8.0 * (buttons.len().saturating_sub(1)) as f32;
    let mut answer = None;
    for (i, b) in buttons.iter().enumerate() {
        let variant = if i == 0 {
            style::Variant::Primary
        } else {
            style::Variant::Secondary
        };
        if kit
            .flat_button(
                &format!("dlg-{i}"),
                variant,
                None,
                b,
                x,
                y,
                bw,
                40.0,
                true,
                "",
            )
            .clicked
        {
            answer = Some(Answer::Button(i));
        }
        x += bw + 8.0;
    }
    answer
}

/// "Which one is your Quest?": one button per authorized device.
fn draw_picker(kit: &mut Kit, size: egui::Vec2, devices: &[Device]) -> Option<Answer> {
    kit.fill(0.0, 0.0, size.x, size.y, style::SURFACE_SOLID);
    kit.text(
        24.0,
        24.0,
        "More than one device is plugged in.",
        style::body(14.0),
        style::TEXT,
    );
    kit.text(
        24.0,
        46.0,
        "Which one is your Quest?",
        style::bold(14.0),
        style::TEXT,
    );
    for (i, d) in devices.iter().take(5).enumerate() {
        if kit
            .flat_button(
                &format!("dev{i}"),
                style::Variant::Secondary,
                Some(style::Icon::Headset),
                &d.label(),
                24.0,
                96.0 + i as f32 * 50.0,
                size.x - 48.0,
                style::MID,
                true,
                "",
            )
            .clicked
        {
            return Some(Answer::Button(i));
        }
    }
    None
}
