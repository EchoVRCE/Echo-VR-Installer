//! The Install page's right-hand column: one card like the left side's, from the hero's
//! top to VERSIONS' bottom. On the PC side, YOUR LIBRARY: where versions go, every
//! installed one as a tile with PLAY and MANAGE, and adding one you already have. On the
//! Quest side, YOUR QUEST: the headset and what is installed on it.

use egui::Color32;

use super::hero::{self, JobView};
use super::install::{myriad, play_version};
use super::{play, server_info, settings, versions, Dashboard};
use crate::core::adb::devices::Status;
use crate::core::launcher::store::InstalledVersion;
use crate::core::paths;
use crate::ui::design::{self, dz, Dr};
use crate::ui::kit::Kit;
use crate::ui::style::Icon;
use crate::ui::widgets::{Tone, BTN_H};

// Geometry in design pixels.
/// The card: level with the hero's top and VERSIONS' bottom.
const CARD: Dr = Dr::new(1318.0, 80.0, 555.0, 966.0);
/// A tile (an installed version, the headset), its gap, its padding and its buttons.
const TILE_H: f32 = 92.0;
const TILE_GAP: f32 = 10.0;
const TILE_PAD: f32 = 14.0;
const ACTION_H: f32 = 40.0;
/// PLAY and MANAGE in a tile.
const PLAY_W: f32 = 96.0;
const MANAGE_W: f32 = 150.0;
const ACTION_GAP: f32 = 8.0;
/// A tile's faint fill, and the line over ALREADY HAVE IT?.
const TILE: Color32 = Color32::from_rgba_premultiplied(8, 8, 8, 8);
const RULE: Color32 = Color32::from_rgba_premultiplied(30, 30, 30, 30);

const EXISTING_TEXT: &str = "Echo VR already on this PC? Add its folder, or the copy in your Meta library, instead of downloading it again.";
const EMPTY_TEXT: &str =
    "Nothing installed yet. Install Echo VR on the left, or add a folder you already have.";

/// The card, as tall as the window lets VERSIONS be.
fn card(kit: &Kit) -> Dr {
    CARD.taller(kit.dy())
}

// ---- YOUR LIBRARY ----

/// YOUR LIBRARY: the install location, every installed version, and ALREADY HAVE IT? at
/// the bottom.
pub(super) fn pc(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    server_info::at_right(kit, |k| {
        let (x, y, w, bottom) = hero::card_frame(k, card(k), "Your library");
        let existing_top = existing(d, k, x, w, bottom);
        let y = location(d, k, x, y, w) + dz(34.0);
        installed(d, k, ctx, x, y, w, existing_top - dz(24.0));
    });
}

/// INSTALL LOCATION: the folder as Settings has it (a field and Browse), and the free
/// space there. Returns the bottom.
fn location(d: &mut Dashboard, kit: &mut Kit, x: f32, y: f32, w: f32) -> f32 {
    kit.caption(x, y, "Install location");
    let mut y = y + dz(30.0);
    let tip = format!("Where new versions are installed:\n{}", d.library_field);
    let bw = kit.button_width("Browse", Some(Icon::Folder), BTN_H);
    if kit.field(
        "install-library",
        &mut d.library_field,
        x,
        y,
        w - bw - dz(14.0),
        BTN_H,
        "",
        false,
        &tip,
    ) {
        settings::set_library(d);
    }
    if kit
        .button(
            "install-lib-browse",
            x + w - bw,
            y,
            bw,
            BTN_H,
            Tone::Dark,
            Some(Icon::Folder),
            "Browse",
            !d.any_job(),
            "Pick the library folder",
        )
        .clicked
    {
        hero::choose_library(d);
    }
    y += BTN_H + dz(12.0);
    let free = d.free_bytes().map_or_else(|| "?".into(), play::gb);
    y + kit.caps_text(
        x,
        y,
        w,
        &format!("{free} free there"),
        14.0,
        design::GREY,
        0.0,
    )
}

/// INSTALLED: a tile per installed version, scrolling between `top` and `bottom`.
fn installed(
    d: &mut Dashboard,
    kit: &mut Kit,
    ctx: &egui::Context,
    x: f32,
    top: f32,
    w: f32,
    bottom: f32,
) {
    kit.caption(x, top, "Installed");
    let top = top + dz(30.0);
    let versions = d.state.versions.clone();
    if versions.is_empty() {
        kit.caps_text(x, top, w, EMPTY_TEXT, 15.8, design::TEXT, 0.0);
        return;
    }
    let pitch = dz(TILE_H + TILE_GAP);
    let list_h = (bottom - top).max(dz(TILE_H));
    let content_h = versions.len() as f32 * pitch - dz(TILE_GAP);
    kit.scroll_area(
        "library",
        x,
        top,
        w + dz(16.0),
        list_h,
        content_h,
        &mut d.library_scroll,
    );
    let scroll = d.library_scroll;
    kit.clipped(x, top, w, list_h, |k| {
        for (i, v) in versions.iter().enumerate() {
            version_tile(d, k, ctx, v, x, top - scroll + i as f32 * pitch, w);
        }
    });
}

/// A tile's background: faint, or VERSIONS' blue with its bar for the selected one.
fn tile(kit: &Kit, x: f32, y: f32, w: f32, h: f32, selected: bool) {
    let r = kit.rect(x, y, w, h);
    let p = kit.ui.painter();
    if selected {
        p.rect_filled(r, dz(6.0), design::BLUE.gamma_multiply(0.28));
        let bar = egui::Rect::from_min_size(r.min, egui::vec2(dz(4.0), r.height()));
        p.rect_filled(bar, dz(2.0), design::BLUE);
    } else {
        p.rect_filled(r, dz(6.0), TILE);
    }
}

/// One installed version: its name in DIN caps with its chips on the right, and under
/// them its folder with PLAY and MANAGE (or the progress of a job on it) on the right.
fn version_tile(
    d: &mut Dashboard,
    k: &mut Kit,
    ctx: &egui::Context,
    v: &InstalledVersion,
    x: f32,
    y: f32,
    w: f32,
) {
    let selected = d.state.selected.as_deref() == Some(v.id.as_str());
    tile(k, x, y, w, dz(TILE_H), selected);
    let (tx, right) = (x + dz(TILE_PAD), x + w - dz(TILE_PAD));

    let chips = chips(d, v);
    let chips_w: f32 = chips.iter().map(|(t, _)| k.chip_width(t) + dz(8.0)).sum();
    let name = k.label_galley(
        &v.name,
        design::din(18.0),
        design::TEXT,
        (right - tx - chips_w - dz(10.0)).max(dz(120.0)),
    );
    let nr = k.put(tx, y + dz(12.0), name);
    let cy = nr.center().y - k.origin.y - dz(13.5);
    let mut cx = right - chips_w + dz(8.0);
    for (t, c) in chips {
        cx += k.chip(cx, cy, t, c) + dz(8.0);
    }

    let ay = y + dz(TILE_H - TILE_PAD - ACTION_H);
    let aw = dz(PLAY_W + ACTION_GAP + MANAGE_W);
    let (sub, color) = folder_line(d, v);
    let sub_w = right - aw - dz(12.0) - tx;
    let g = myriad(k, &sub, design::myriad(16.0), color, sub_w, true);
    let gy = ay + (dz(ACTION_H) - g.size().y) / 2.0;
    k.put(tx, gy, g);
    actions(d, k, ctx, v, right - aw, ay);
}

/// An installed version's chips: selected, added from elsewhere, patched.
fn chips(d: &Dashboard, v: &InstalledVersion) -> Vec<(&'static str, Color32)> {
    let mut chips = Vec::new();
    if d.state.selected.as_deref() == Some(v.id.as_str()) {
        chips.push(("Selected", design::QUEST_ON));
    }
    if v.external {
        chips.push(("Existing folder", design::QUEST_OFF));
    }
    if v.patched {
        chips.push(("Patched", design::QUEST_OFF));
    }
    chips
}

/// Whether its game files are there.
fn files_ok(d: &Dashboard, v: &InstalledVersion) -> bool {
    d.demo || paths::has_echo_install(&v.root)
}

/// Its folder, or that the files are gone.
fn folder_line(d: &Dashboard, v: &InstalledVersion) -> (String, Color32) {
    if files_ok(d, v) {
        (v.root.clone(), design::SUBTLE)
    } else {
        (format!("Files missing: {}", v.root), design::DANGER)
    }
}

/// PLAY and MANAGE from `x`, or the progress of a job on the version in their place.
fn actions(
    d: &mut Dashboard,
    k: &mut Kit,
    ctx: &egui::Context,
    v: &InstalledVersion,
    x: f32,
    y: f32,
) {
    let h = dz(ACTION_H);
    if let Some(job) = hero::job_view(d, &v.id) {
        job_bar(d, k, &job, x, y, dz(PLAY_W + ACTION_GAP + MANAGE_W), h);
        return;
    }
    let ok = files_ok(d, v);
    let tip = if ok {
        "Select this version and go to Play"
    } else {
        "The game files are missing"
    };
    let play_w = dz(PLAY_W);
    if k.button(
        &format!("play-{}", v.id),
        x,
        y,
        play_w,
        h,
        Tone::Go,
        None,
        "Play",
        ok,
        tip,
    )
    .clicked
    {
        play_version(d, &v.id);
    }
    let mx = x + play_w + dz(ACTION_GAP);
    versions::manage_menu(d, k, ctx, v, mx, y, dz(MANAGE_W), h);
}

/// A job's progress bar and a ✕ that cancels it, `w` wide and `h` high in all.
fn job_bar(d: &mut Dashboard, k: &mut Kit, job: &JobView, x: f32, y: f32, w: f32, h: f32) {
    let bw = w - h - 6.0;
    k.progress_bar(x, y, bw, h, job.fraction, &job.step());
    let tip = if job.cancelling {
        "Stopping…".to_string()
    } else {
        format!("Stop {}", job.title.to_lowercase())
    };
    if k.button(
        &format!("cancel-{}", job.id),
        x + bw + 6.0,
        y,
        h,
        h,
        Tone::Dark,
        Some(Icon::Close),
        "",
        !job.cancelling,
        &tip,
    )
    .clicked
    {
        d.cancel_job(&job.id);
    }
}

/// ALREADY HAVE IT? at the card's bottom, under a line: what it is for, then adding a
/// folder and (on Windows) finding the copy in the Meta library. Returns its top.
fn existing(d: &mut Dashboard, kit: &mut Kit, x: f32, w: f32, bottom: f32) -> f32 {
    let mut buttons = vec![(
        "add-existing",
        Icon::Folder,
        "Add existing folder",
        "Use an Echo VR install that is already on this PC",
    )];
    if cfg!(windows) || d.demo {
        buttons.push((
            "find-meta",
            Icon::Headset,
            "Find Meta install",
            "Add Echo VR from your Meta (Oculus) library",
        ));
    }
    let gap = dz(14.0);
    let n = buttons.len() as f32;
    let buttons_top = bottom - n * BTN_H - (n - 1.0) * gap;
    let text_h: f32 = kit
        .caps_block(EXISTING_TEXT, 14.0, design::GREY, w)
        .iter()
        .map(|g| g.size().y)
        .sum();
    let text_y = buttons_top - dz(18.0) - text_h;
    let caption_y = text_y - dz(28.0);
    let rule_y = caption_y - dz(24.0);
    let rule = kit.rect(x, rule_y, w, 0.0);
    kit.ui
        .painter()
        .hline(rule.x_range(), rule.min.y, egui::Stroke::new(1.0, RULE));
    kit.caption(x, caption_y, "Already have it?");
    kit.caps_text(x, text_y, w, EXISTING_TEXT, 14.0, design::GREY, 0.0);
    let mut by = buttons_top;
    for (key, icon, label, tip) in buttons {
        if kit
            .button(
                key,
                x,
                by,
                w,
                BTN_H,
                Tone::Dark,
                Some(icon),
                label,
                true,
                tip,
            )
            .clicked
        {
            match key {
                "add-existing" => versions::add_existing(d),
                _ => versions::find_meta(d),
            }
        }
        by += BTN_H + gap;
    }
    rule_y
}

// ---- YOUR QUEST ----

/// YOUR QUEST: the headset (its connection, and Check again) and what is installed on it.
pub(super) fn quest(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    server_info::at_right(kit, |k| {
        let (x, y, w, _) = hero::card_frame(k, card(k), "Your Quest");
        let h = headset(d);
        let (tx, right) = (x + dz(TILE_PAD), x + w - dz(TILE_PAD));

        // The headset: its name and connection, then how it connects and Check again.
        k.caption(x, y, "Headset");
        let ty = y + dz(30.0);
        tile(k, x, ty, w, dz(TILE_H), false);
        let chip_w = k.chip_width(h.conn);
        let name = h
            .device
            .clone()
            .unwrap_or_else(|| if h.ready { "Quest" } else { "No headset" }.into());
        let name = k.label_galley(
            &name,
            design::din(18.0),
            design::TEXT,
            (right - tx - chip_w - dz(10.0)).max(dz(120.0)),
        );
        let nr = k.put(tx, ty + dz(12.0), name);
        k.chip(
            right - chip_w,
            nr.center().y - k.origin.y - dz(13.5),
            h.conn,
            h.color,
        );
        let ay = ty + dz(TILE_H - TILE_PAD - ACTION_H);
        let bw = k
            .button_width("Check again", Some(Icon::Refresh), dz(ACTION_H))
            .max(dz(PLAY_W));
        let note = k.label_galley(
            "Over USB, with developer mode on",
            design::din(14.0),
            design::GREY,
            right - bw - dz(12.0) - tx,
        );
        let gy = ay + (dz(ACTION_H) - note.size().y) / 2.0;
        k.put(tx, gy, note);
        if k.button(
            "quest-recheck",
            right - bw,
            ay,
            bw,
            dz(ACTION_H),
            Tone::Dark,
            Some(Icon::Refresh),
            "Check again",
            !d.quest_conn.checking && !d.quest_busy,
            "Look for your Quest over USB",
        )
        .clicked
        {
            d.check_quest(ctx, true);
        }

        // Echo VR on it: the build's file, or why there is none to show.
        let ey = ty + dz(TILE_H + 34.0);
        k.caption(x, ey, "Echo VR");
        let ey = ey + dz(30.0);
        let (text, color) = match &h.echo {
            Ok(apk) => (format!("Installed: {apk}"), design::TEXT),
            Err(note) => (note.to_string(), design::GREY),
        };
        let text_h: f32 = k
            .caps_block(&text, 16.0, color, right - tx)
            .iter()
            .map(|g| g.size().y)
            .sum();
        tile(k, x, ey, w, text_h + dz(2.0 * 22.0), false);
        k.caps_text(tx, ey + dz(22.0), right - tx, &text, 16.0, color, 0.0);
    });
}

/// What the headset card says.
struct Headset {
    ready: bool,
    conn: &'static str,
    /// The connection's chip colour.
    color: Color32,
    device: Option<String>,
    /// The installed build's label, or why there is none to show.
    echo: Result<String, &'static str>,
}

fn headset(d: &Dashboard) -> Headset {
    let (conn, color) = match (d.quest_conn.checking, d.quest_conn.status) {
        (true, _) => ("Checking…", design::QUEST_OFF),
        (_, Some(Status::Ready)) => ("Connected", design::QUEST_ON),
        (_, Some(Status::Unauthorized)) => ("Allow this PC in the headset", design::QUEST_WARN),
        (_, Some(Status::Ambiguous)) => ("Several devices: pick yours", design::QUEST_WARN),
        (_, Some(Status::None)) => ("Not connected", design::QUEST_OFF),
        (_, None) => ("Not checked yet", design::QUEST_OFF),
    };
    let ready = d.quest_conn.status == Some(Status::Ready);
    let info = d.quest_info.clone().filter(|_| ready);
    Headset {
        ready,
        conn,
        color,
        device: info.as_ref().and_then(|i| i.device.clone()),
        echo: match &info {
            Some(i) if i.installed => Ok(i.version_label()),
            Some(_) => Err("Not installed on this Quest"),
            None => Err("Connect the headset to see what is installed"),
        },
    }
}

// ---- for VERSIONS ----

/// Where the version list came from and Refresh, right-aligned on the title row of the
/// VERSIONS card `r` (design pixels).
pub(super) fn list_source(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context, r: Dr) {
    let source = match &d.catalog {
        _ if d.catalog_loading => "Loading…",
        None => "Loading…",
        Some(c) if c.builtin => "Built-in list",
        Some(_) => "Online list",
    };
    let caps = |kit: &Kit, text: &str, color: Color32, underline: bool| {
        kit.spaced_galley(
            &text.to_uppercase(),
            design::din(14.0),
            color,
            dz(0.5),
            underline,
        )
    };
    let y = dz(r.y + 28.0);
    let link_w = caps(kit, "Refresh", design::TEXT, true).size().x;
    let lx = dz(r.right() - 22.0) - link_w;
    let label = caps(kit, source, design::GREY, false);
    kit.put(lx - dz(18.0) - label.size().x, y, label);
    if d.catalog_loading {
        kit.put(lx, y, caps(kit, "Refresh", design::GREY, false));
    } else if kit
        .link(
            "lib-refresh",
            lx,
            y,
            "Refresh",
            14.0,
            "Load the list of versions again",
        )
        .clicked
    {
        d.refresh_catalog(ctx);
    }
}
