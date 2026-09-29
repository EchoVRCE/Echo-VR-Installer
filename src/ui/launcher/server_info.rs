//! SERVER INFO: the Discord status post, laid out the way Discord shows an embed: its
//! description, inline fields three to a row, the image (the world map) and the footer
//! with its time.

use egui::{pos2, vec2, Color32, Rect};

use super::Dashboard;
use crate::core::launcher::feed::{self, Embed};
use crate::ui::design::{self, dz, Dr};
use crate::ui::kit::Kit;
use crate::ui::markdown::{self, Look};

const PANEL: Dr = Dr::new(1318.0, 15.0, 555.0, 1031.0);
/// Content column (the image and footer use all of it).
const X: f32 = 1349.0;
const W: f32 = 491.0;
/// Text wraps where Discord wraps an embed; inline fields sit in columns this far apart.
const TEXT_W: f32 = 372.0;
const FIELD_PITCH: f32 = 124.0;
const TITLE_Y: f32 = 48.0;
const BODY_Y: f32 = 98.0;
/// The image sits above the footer, at most this tall.
const MAP_MAX_H: f32 = 280.0;
const FOOTER_Y: f32 = 1000.0;

fn look() -> Look {
    Look {
        font: design::myriad(20.0),
        bold: design::myriad_bold(20.0),
        mono: egui::FontId::monospace(dz(14.5)),
        small: design::myriad(19.0),
        color: design::BODY,
        strong: design::HEADING,
        subtle: design::SUBTLE,
        link: design::LINK,
        chip: Color32::from_rgb(16, 4, 44),
        chip_rim: Color32::from_rgb(46, 32, 84),
        line_gap: dz(8.0),
        paragraph_gap: dz(10.0),
        bullet_indent: dz(20.0),
        uppercase: false,
        dash_bullets: false,
    }
}

pub(super) fn show(d: &Dashboard, kit: &mut Kit) {
    kit.image_d("panel_bg.png", PANEL);
    let embed = d.feed.status.as_ref().and_then(|s| s.embeds.first());
    let title = embed
        .and_then(|e| e.title.clone())
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "Server info".into());
    let g = kit.spaced_galley(
        &title.to_uppercase(),
        design::din(24.0),
        design::TEXT,
        dz(1.2),
        false,
    );
    kit.put(dz(1342.0), dz(TITLE_Y), g);

    let Some(embed) = embed else {
        let text = if d.feed.status_failed {
            "Server info is unavailable right now."
        } else {
            "Loading server info..."
        };
        markdown::draw(kit, dz(X), dz(BODY_Y), dz(W), text, &look());
        return;
    };
    let map = d.feed.texture(embed.image.as_deref()).cloned();
    let (map_rect, content_bottom) = match &map {
        Some(tex) => {
            let [tw, th] = tex.size();
            let aspect = th.max(1) as f32 / tw.max(1) as f32;
            let h = (W * aspect).min(MAP_MAX_H);
            let w = h / aspect;
            let top = FOOTER_Y - 24.0 - h;
            (Some(Dr::new(X + (W - w) / 2.0, top, w, h)), top - 16.0)
        }
        None => (None, FOOTER_Y - 16.0),
    };
    let (x, y0) = (dz(X), dz(BODY_Y));
    kit.clipped(
        x - dz(8.0),
        y0,
        dz(W + 16.0),
        dz(content_bottom) - y0,
        |kit| {
            body(kit, embed, x, y0, dz(TEXT_W));
        },
    );
    if let (Some(tex), Some(r)) = (&map, map_rect) {
        kit.ui
            .painter()
            .image(tex.id(), kit.drect(r), uv(), Color32::WHITE);
    }
    footer(kit, embed);
}

fn uv() -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))
}

/// The description, then the fields: inline ones three to a row, others full width.
fn body(kit: &Kit, e: &Embed, x: f32, mut y: f32, w: f32) {
    let look = look();
    if let Some(desc) = e.description.as_deref().filter(|t| !t.trim().is_empty()) {
        y += markdown::draw(kit, x, y, w, desc, &look) + dz(22.0);
    }
    let name_look = Look {
        font: design::myriad_bold(20.0),
        color: design::HEADING,
        ..look.clone()
    };
    let value_look = Look {
        line_gap: dz(4.0),
        ..look.clone()
    };
    let pitch = dz(FIELD_PITCH);
    let mut i = 0;
    while i < e.fields.len() {
        let row: Vec<_> = if e.fields[i].inline {
            e.fields[i..]
                .iter()
                .take_while(|f| f.inline)
                .take(3)
                .collect()
        } else {
            vec![&e.fields[i]]
        };
        let mut row_h: f32 = 0.0;
        for (c, f) in row.iter().enumerate() {
            let cx = x + c as f32 * pitch;
            // The last column may run to the panel's edge.
            let cw = if !f.inline {
                w
            } else if c == 2 {
                dz(W) - 2.0 * pitch
            } else {
                pitch - dz(6.0)
            };
            let nh = markdown::draw(kit, cx, y, cw, &f.name, &name_look);
            let vh = markdown::draw(kit, cx, y + nh + dz(4.0), cw, &f.value, &value_look);
            row_h = row_h.max(nh + dz(4.0) + vh);
        }
        y += row_h + dz(12.0);
        i += row.len();
    }
}

/// "Last updated • Today at 12:32 PM".
fn footer(kit: &Kit, e: &Embed) {
    let when = e
        .timestamp
        .as_deref()
        .and_then(feed::parse_time)
        .map(feed::footer_time);
    let text = match (e.footer.as_deref().filter(|f| !f.is_empty()), when) {
        (Some(f), Some(t)) => format!("{f} • {t}"),
        (Some(f), None) => f.to_string(),
        (None, Some(t)) => t,
        (None, None) => return,
    };
    let g = kit.spaced_galley(&text, design::myriad(18.0), design::HEADING, 0.0, false);
    let r = kit.drect(Dr::new(X, FOOTER_Y, W, 24.0));
    kit.ui.painter().galley(
        pos2(r.min.x, r.center().y - g.size().y / 2.0) + vec2(0.0, 0.0),
        g,
        design::HEADING,
    );
}
