//! Fonts and colours, copied from the Java Swing code.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};

pub const CONTHRAX: &str = "conthrax";
pub const ARIAL: &str = "arial";
pub const ARIAL_BOLD: &str = "arial-bold";
/// The launcher design's condensed DIN caps (status bar, info line, card text).
pub const DIN: &str = "din";
/// The launcher design's Myriad (SERVER INFO panel).
pub const MYRIAD: &str = "myriad";
pub const MYRIAD_BOLD: &str = "myriad-bold";

/// The Swing UI's own font.
pub fn conthrax(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(CONTHRAX.into()))
}

/// Java's logical "Arial"; Liberation Sans is metric-compatible and freely licensed.
pub fn arial(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ARIAL.into()))
}

pub fn arial_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ARIAL_BOLD.into()))
}

pub fn din(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(DIN.into()))
}

pub fn myriad(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MYRIAD.into()))
}

pub fn myriad_bold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(MYRIAD_BOLD.into()))
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    defs.font_data.insert(
        CONTHRAX.into(),
        FontData::from_static(include_bytes!("../../assets/fonts/conthrax-sb.otf")).into(),
    );
    defs.font_data.insert(
        ARIAL.into(),
        FontData::from_static(include_bytes!(
            "../../assets/fonts/LiberationSans-Regular.ttf"
        ))
        .into(),
    );
    defs.font_data.insert(
        ARIAL_BOLD.into(),
        FontData::from_static(include_bytes!("../../assets/fonts/LiberationSans-Bold.ttf")).into(),
    );
    defs.font_data.insert(
        DIN.into(),
        FontData::from_static(include_bytes!("../../assets/fonts/dmcaps.ttf")).into(),
    );
    defs.font_data.insert(
        MYRIAD.into(),
        FontData::from_static(include_bytes!("../../assets/fonts/myriad-medium.ttf")).into(),
    );
    defs.font_data.insert(
        MYRIAD_BOLD.into(),
        FontData::from_static(include_bytes!("../../assets/fonts/myriad-bold.ttf")).into(),
    );
    // egui's bundled fonts stay behind ours as glyph fallbacks (✓, arrows, emoji).
    let fallbacks: Vec<String> = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    for name in [CONTHRAX, ARIAL, ARIAL_BOLD, DIN, MYRIAD, MYRIAD_BOLD] {
        let mut chain = vec![name.to_string()];
        match name {
            // These lack symbols like "…", "·" or "•".
            CONTHRAX | DIN | MYRIAD => chain.push(ARIAL.into()),
            MYRIAD_BOLD => chain.push(ARIAL_BOLD.into()),
            _ => {}
        }
        chain.extend(fallbacks.iter().cloned());
        defs.families.insert(FontFamily::Name(name.into()), chain);
    }
    // Plain egui widgets (TextEdit caret metrics etc.) default to Arial too.
    if let Some(p) = defs.families.get_mut(&FontFamily::Proportional) {
        p.insert(0, ARIAL.into());
    }
    ctx.set_fonts(defs);
}

pub fn install_style(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        style.interaction.selectable_labels = false;
        style.visuals = egui::Visuals::dark();
        style.visuals.text_cursor.stroke = egui::Stroke::new(1.5, Color32::WHITE);
        style.visuals.selection.bg_fill = Color32::from_rgb(70, 110, 180);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, Color32::WHITE);
        style.spacing.item_spacing = egui::vec2(0.0, 0.0);
    });
}

/// `new Color(r, g, b, a)` -- Java colours are unmultiplied.
pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied_const(r, g, b, a)
}

pub const WHITE: Color32 = Color32::WHITE;
pub const BLACK: Color32 = Color32::BLACK;

pub const BOX_BORDER: Color32 = rgba(50, 50, 50, 150);

pub const STATUS_DONE: Color32 = Color32::from_rgb(40, 130, 40);

pub const CHIP_CURRENT_BG: Color32 = Color32::from_rgb(0, 180, 0);
pub const CHIP_UPCOMING_BG: Color32 = Color32::from_rgb(40, 40, 40);

pub const BUTTON_TEXT: Color32 = Color32::from_rgb(230, 230, 230);
pub const BUTTON_TEXT_HOVER: Color32 = Color32::from_rgb(250, 250, 250);

pub const FIELD_BG_INVALID: Color32 = rgba(150, 45, 45, 210);
pub const PLACEHOLDER: Color32 = Color32::from_rgb(170, 170, 170);

pub const PROGRESS_BG: Color32 = rgba(255, 255, 255, 200);

pub const MARK_OK: Color32 = Color32::from_rgb(80, 255, 0);
pub const MARK_BAD: Color32 = Color32::from_rgb(255, 80, 80);
