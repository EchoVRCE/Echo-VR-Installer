//! SERVER INFO, laid out like the Discord status post it replaces: servers and how busy
//! they are, per region, players over time, the matches per mode three to a row, the
//! servers on a world map, and when it was updated.

use egui::{pos2, Color32};

use super::Dashboard;
use crate::core::launcher::feed::{self, Location, Servers, Slot};
use crate::ui::design::{self, dz, Dr};
use crate::ui::kit::Kit;
use crate::ui::markdown::{self, Look};

const PANEL: Dr = Dr::new(1318.0, 15.0, 555.0, 1031.0);
/// Content column; text wraps where Discord wraps an embed.
const X: f32 = 1349.0;
const TEXT_W: f32 = 372.0;
/// The mode columns' spacing.
const FIELD_PITCH: f32 = 124.0;
const TITLE_Y: f32 = 48.0;
const BODY_Y: f32 = 98.0;
/// world_map.png (see scripts/world_map.py): Miller projection from MAX_LAT to MIN_LAT.
const MAP: Dr = Dr::new(1347.0, 707.0, 493.0, 281.0);
const MIN_LAT: f32 = -73.5;
const MAX_LAT: f32 = 83.6;
const LAND: Color32 = Color32::from_rgba_unmultiplied_const(235, 225, 245, 46);
const SERVER_DOT: Color32 = Color32::from_rgba_unmultiplied_const(255, 255, 255, 225);
const BUSY_DOT: Color32 = Color32::from_rgb(255, 86, 120);
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
    let g = kit.spaced_galley(
        "SERVER INFO",
        design::din(24.0),
        design::TEXT,
        dz(1.2),
        false,
    );
    kit.put(dz(1342.0), dz(TITLE_Y), g);
    kit.image_tinted("world_map.png", MAP, LAND);

    let Some(s) = &d.feed.status else {
        let text = if d.feed.status_failed {
            "Server info is unavailable right now."
        } else {
            "Loading server info..."
        };
        markdown::draw(kit, dz(X), dz(BODY_Y), dz(TEXT_W), text, &look());
        return;
    };
    let (x, y0) = (dz(X), dz(BODY_Y));
    kit.clipped(
        x - dz(8.0),
        y0,
        dz(TEXT_W + 130.0),
        dz(MAP.y - 10.0) - y0,
        |kit| {
            let y = y0 + markdown::draw(kit, x, y0, dz(TEXT_W), &summary(s), &look()) + dz(22.0);
            modes(kit, s, x, y);
        },
    );
    map(kit, &s.locations);
    footer(kit, s);
}

/// Servers, regions and players, as the status post words them.
fn summary(s: &Servers) -> String {
    let n = |v: u32| format!("`{v}`");
    let p = |v: u32| format!("`{v}%`");
    let status = if s.status.is_empty() { "?" } else { &s.status };
    let startup = s
        .started_at
        .as_deref()
        .and_then(feed::parse_time)
        .map(|t| format!(" | Startup: <t:{}:f>", t.unix_timestamp()))
        .unwrap_or_default();
    let (sv, u, r, pl) = (s.servers, s.usage, &s.regions, &s.players);
    let since = pl.since.as_deref().and_then(feed::parse_time);
    // A window the history doesn't cover yet says since when it counts.
    let counted = |days: i64| match since {
        Some(t) if t > time::OffsetDateTime::now_utc() - time::Duration::days(days) => {
            format!(" (since {} {})", short_month(t), t.day())
        }
        _ => String::new(),
    };
    [
        "-# Total | Public | Private".to_string(),
        format!(
            "- Online: {} | {} | {}",
            n(sv.total),
            n(sv.public),
            n(sv.private)
        ),
        format!(
            "- Usage: {} | {} | {}",
            p(u.total),
            p(u.public),
            p(u.private)
        ),
        format!("- Status: `{status}`{startup}"),
        "__**Region**__".into(),
        "-# NA | EU | OCE".into(),
        format!(
            "- Online: {} | {} | {}",
            n(r.na.servers),
            n(r.eu.servers),
            n(r.oce.servers)
        ),
        format!(
            "- Usage: {} | {} | {}",
            p(r.na.usage),
            p(r.eu.usage),
            p(r.oce.usage)
        ),
        "__**Players**__".into(),
        format!("- 30 Days: {}{}", n(pl.last_30d), counted(30)),
        format!("- 24 Hours: {}{}", n(pl.last_24h), counted(1)),
        format!("- Last Hour: {}", n(pl.last_hour)),
        format!("- Online: {}", n(pl.online)),
    ]
    .join("\n")
}

fn short_month(t: time::OffsetDateTime) -> String {
    t.month().to_string().chars().take(3).collect()
}

/// Lobby, Arena and Combat, public then private: players/limit (spectators).
fn modes(kit: &Kit, s: &Servers, x: f32, mut y: f32) {
    let look = look();
    let name_look = Look {
        font: design::myriad_bold(20.0),
        color: design::HEADING,
        ..look.clone()
    };
    let m = &s.modes;
    let rows = [
        [
            ("Lobby", m.lobby.public),
            ("Arena", m.arena.public),
            ("Combat", m.combat.public),
        ],
        [
            ("Lobby Private", m.lobby.private),
            ("Arena Private", m.arena.private),
            ("Combat Private", m.combat.private),
        ],
    ];
    let pitch = dz(FIELD_PITCH);
    for row in rows {
        let mut row_h: f32 = 0.0;
        for (c, (name, slot)) in row.into_iter().enumerate() {
            let cx = x + c as f32 * pitch;
            let w = if c == 2 {
                pitch + dz(60.0)
            } else {
                pitch - dz(6.0)
            };
            let nh = markdown::draw(kit, cx, y, w, name, &name_look);
            let (value, color) = value(slot);
            let value_look = Look {
                color,
                ..look.clone()
            };
            let vh = markdown::draw(kit, cx, y + nh + dz(4.0), w, &value, &value_look);
            row_h = row_h.max(nh + dz(4.0) + vh);
        }
        y += row_h + dz(12.0);
    }
}

fn value(slot: Slot) -> (String, Color32) {
    if slot.matches == 0 {
        return ("n/a".into(), design::BODY);
    }
    let mut v = format!("{}/{}", slot.players, slot.limit);
    if slot.spectators > 0 {
        v.push_str(&format!(" ({})", slot.spectators));
    }
    (v, design::LINK)
}

/// Where a place lands on the map (the Miller projection of world_map.png).
fn project(lat: f32, lon: f32) -> (f32, f32) {
    let miller = |lat: f32| {
        1.25 * (std::f32::consts::FRAC_PI_4 + 0.4 * lat.to_radians())
            .tan()
            .ln()
    };
    let (top, bottom) = (miller(MAX_LAT), miller(MIN_LAT));
    let lat = lat.clamp(MIN_LAT, MAX_LAT);
    (
        MAP.x + (lon + 180.0) / 360.0 * MAP.w,
        MAP.y + (top - miller(lat)) / (top - bottom) * MAP.h,
    )
}

/// A dot per place with servers, larger with more of them; pink where a match runs.
fn map(kit: &Kit, places: &[Location]) {
    let p = kit.ui.painter();
    for l in places {
        let (x, y) = project(l.lat, l.lon);
        let c = kit.origin + egui::vec2(dz(x), dz(y));
        let r = dz(1.7 + (l.servers as f32).sqrt() * 0.35);
        let color = if l.matches > 0 { BUSY_DOT } else { SERVER_DOT };
        p.circle_filled(c, r * 1.8, color.gamma_multiply(0.15));
        p.circle_filled(c, r, color);
    }
}

/// "Last updated • Today at 12:32 PM".
fn footer(kit: &Kit, s: &Servers) {
    let Some(when) = s
        .updated_at
        .as_deref()
        .and_then(feed::parse_time)
        .map(feed::footer_time)
    else {
        return;
    };
    let g = kit.spaced_galley(
        &format!("Last updated • {when}"),
        design::myriad(18.0),
        design::HEADING,
        0.0,
        false,
    );
    let r = kit.drect(Dr::new(X, FOOTER_Y, TEXT_W, 24.0));
    kit.ui.painter().galley(
        pos2(r.min.x, r.center().y - g.size().y / 2.0),
        g,
        design::HEADING,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_projection_corners() {
        let (x, y) = project(MAX_LAT, -180.0);
        assert!((x - MAP.x).abs() < 0.01 && (y - MAP.y).abs() < 0.01);
        let (x, y) = project(MIN_LAT, 180.0);
        assert!((x - MAP.right()).abs() < 0.01 && (y - MAP.bottom()).abs() < 0.01);
        // The equator sits below the middle (more land in the north is shown).
        assert!(project(0.0, 0.0).1 > MAP.y + MAP.h / 2.0);
    }
}
