//! Versions page: installed versions and the catalogue as rows in the section box, with
//! PC | Quest chips and Add folder / Refresh in the bottom bar.

use super::{header, para, Dashboard, JobResult, Page, CX, HEADER_Y, IW, IX};
use crate::core::error::UiError;
use crate::core::launcher::catalog::{Platform, VersionEntry};
use crate::core::launcher::store::InstalledVersion;
use crate::core::launcher::versions;
use crate::core::{paths, platform};
use crate::ui::dialogs::Icon as DlgIcon;
use crate::ui::frame::{self, Chip, CONTENT_W, CONTENT_X, SECTION_H, SECTION_Y};
use crate::ui::kit::{Btn, Kit};
use crate::ui::launcher::Open;
use crate::ui::parts;
use crate::ui::style;
use crate::ui::theme;

const REMOVE_KEY: &str = "remove-version";
const REPAIR_KEY: &str = "repair-version";
const LIST_Y: f32 = SECTION_Y + 8.0;
const LIST_H: f32 = SECTION_H - 16.0;
const ROW_H: f32 = 46.0;
const ROW_GAP: f32 = 8.0;
/// A small section title: the header banner at 300 wide.
const TITLE_H: f32 = 40.0;
/// Right edge of a row's buttons.
const RIGHT: f32 = IX + IW - 11.0;

pub(super) fn ask_repair(d: &mut Dashboard, id: &str, msg: &str) {
    d.pending_repair = Some(id.to_string());
    d.dialogs
        .confirm(REPAIR_KEY, "Verify", msg, DlgIcon::Warning);
}

fn job_err(e: anyhow::Error, title: &str) -> JobResult {
    if crate::core::http::is_cancelled(&e) {
        JobResult::Failed(None)
    } else {
        JobResult::Failed(Some(UiError::from_anyhow(&e, title)))
    }
}

pub(super) fn update(d: &mut Dashboard, ctx: &egui::Context, v: InstalledVersion) {
    let id = v.id.clone();
    d.start_job(
        ctx,
        &id,
        "Checking for updates...",
        move |cancel, on| match versions::update(&v, cancel, on) {
            Ok(()) => JobResult::Updated,
            Err(e) => job_err(e, "Update Failed"),
        },
    );
}

fn verify(d: &mut Dashboard, ctx: &egui::Context, v: InstalledVersion) {
    let id = v.id.clone();
    d.start_job(
        ctx,
        &id,
        "Verifying game files...",
        move |_, on| match versions::verify(&v, on) {
            Ok(bad) => JobResult::Verified(bad),
            Err(e) => job_err(e, "Verify Failed"),
        },
    );
}

pub(super) fn install(d: &mut Dashboard, ctx: &egui::Context, e: VersionEntry) {
    let library = d.state.library.clone();
    let id = e.id.clone();
    d.start_job(
        ctx,
        &id,
        "Preparing download...",
        move |cancel, on| match versions::install(&e, &library, cancel, on) {
            Ok(v) => JobResult::Installed(v),
            Err(err) => job_err(err, "Install Failed"),
        },
    );
}

pub(super) fn add_existing(d: &mut Dashboard) {
    let Some(p) = parts::choose_folder() else {
        return;
    };
    let root = paths::resolve_install_root(&p);
    if !paths::has_echo_install(&root) {
        d.dialogs.error(
            "Echo VR not found",
            "That folder doesn't contain Echo VR (ready-at-dawn-echo-arena\\bin\\win10\\echovr.exe).",
            Default::default(),
        );
    } else if let Some(id) = d.state.add_external(&root, None) {
        d.state.selected = Some(id);
        d.save();
    } else {
        d.dialogs
            .info("Already added", "This install is already in the list.");
    }
}

fn remove(d: &mut Dashboard, id: &str) {
    let Some(v) = d.state.version(id).cloned() else {
        return;
    };
    if !v.external {
        if let Err(e) = versions::remove(&v, &d.state.library) {
            d.dialogs
                .error("Remove Failed", &format!("{e:#}"), Default::default());
            return;
        }
    }
    d.state.versions.retain(|x| x.id != id);
    if d.state.selected.as_deref() == Some(id) {
        d.state.selected = d.state.versions.first().map(|v| v.id.clone());
    }
    d.save();
}

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    if let Some(a) = d.dialogs.take(REMOVE_KEY) {
        if let (true, Some(id)) = (a.is_yes(), d.pending_remove.take()) {
            remove(d, &id);
        }
    }
    if let Some(a) = d.dialogs.take(REPAIR_KEY) {
        let v = d
            .pending_repair
            .take()
            .and_then(|id| d.state.version(&id).cloned());
        if let (true, Some(v)) = (a.is_yes(), v) {
            update(d, ctx, v);
        }
    }

    bottom_bar(d, kit, ctx);
    if d.versions_platform == Platform::Quest {
        return quest(kit);
    }

    let installed: Vec<InstalledVersion> = d.state.versions.clone();
    let available: Vec<VersionEntry> = d
        .catalog
        .as_ref()
        .map(|c| c.pc().cloned().collect())
        .unwrap_or_default();
    let content_h = 2.0 * TITLE_H
        + 12.0
        + (installed.len().max(1) + available.len().max(1)) as f32 * (ROW_H + ROW_GAP);
    kit.scroll(
        CONTENT_X,
        LIST_Y,
        CONTENT_W,
        LIST_H,
        content_h,
        &mut d.versions_scroll,
    );
    let scroll = d.versions_scroll;
    kit.clipped(CONTENT_X, LIST_Y, CONTENT_W, LIST_H, |k| {
        let mut y = LIST_Y - scroll;
        title(k, "Installed versions", y);
        y += TITLE_H;
        if installed.is_empty() {
            row_box(k, y);
            k.text_fit_center(
                IX,
                y,
                IW,
                ROW_H,
                "Nothing installed yet. Install a version below, or add a folder you already have.",
                theme::arial(13.0),
                style::TEXT,
            );
            y += ROW_H + ROW_GAP;
        }
        for v in &installed {
            installed_row(d, k, ctx, v, y);
            y += ROW_H + ROW_GAP;
        }
        y += 12.0;
        let source = match &d.catalog {
            None => "Available (loading...)",
            Some(c) if c.builtin => "Available (built-in list)",
            _ => "Available",
        };
        title(k, source, y);
        y += TITLE_H;
        for e in &available {
            available_row(d, k, ctx, e, y);
            y += ROW_H + ROW_GAP;
        }
    });
    None
}

/// Add folder | PC · Quest | Refresh, laid out like the wizards' Back | chips | Next.
fn bottom_bar(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    let labels = ["PC", "Quest"];
    let states = if d.versions_platform == Platform::Pc {
        [Chip::Current, Chip::Upcoming]
    } else {
        [Chip::Upcoming, Chip::Current]
    };
    let (_, total) = frame::chip_widths(kit, &labels);
    let chips_x = CONTENT_X + ((CONTENT_W - total) / 2.0).floor();
    let tips = ["Echo VR versions on this PC", "Echo VR on your Quest"];
    if let Some(i) = frame::chips(
        kit,
        "versions-platform",
        chips_x,
        &labels,
        &states,
        false,
        &tips,
    ) {
        d.versions_platform = if i == 0 {
            Platform::Pc
        } else {
            Platform::Quest
        };
    }
    if d.versions_platform == Platform::Quest {
        return;
    }
    let bw = Btn::Small.w();
    let y = frame::bar_button_y();
    let left_gap = chips_x - CONTENT_X;
    let right_gap = CONTENT_X + CONTENT_W - chips_x - total;
    if kit.button(
        "add-existing",
        Btn::Small,
        "Add folder",
        11.0,
        CONTENT_X + ((left_gap - bw) / 2.0).floor(),
        y,
        true,
        "Use an Echo VR install that is already on this PC",
    ) {
        add_existing(d);
    }
    if kit.button(
        "refresh",
        Btn::Small,
        "Refresh",
        11.0,
        chips_x + total + ((right_gap - bw) / 2.0).floor(),
        y,
        !d.catalog_loading,
        "Reload the list of available versions",
    ) {
        d.refresh_catalog(ctx);
    }
}

fn title(k: &Kit, text: &str, y: f32) {
    k.header(text, CX - 150.0, y, 300.0, TITLE_H - 6.0);
}

/// A row's box: the installer's `SpecialLabel` blue-grey, rounded like the boxes.
fn row_box(k: &Kit, y: f32) {
    k.round_box(
        IX,
        y,
        IW,
        ROW_H,
        15.0,
        theme::LABEL_BG,
        Some(theme::BOX_BORDER),
    );
}

/// Name (Conthrax) over a detail line (Arial); returns where badges may start.
fn row_base(k: &Kit, y: f32, name: &str, sub: &str, sub_color: egui::Color32) -> f32 {
    row_box(k, y);
    let font = theme::conthrax(12.0);
    k.text_fit(IX + 16.0, y + 7.0, 360.0, name, font.clone(), style::TEXT);
    k.text_fit(
        IX + 16.0,
        y + 26.0,
        470.0,
        sub,
        theme::arial(12.0),
        sub_color,
    );
    IX + 16.0 + k.text_width(name, font).min(360.0) + 10.0
}

/// A running job in place of a row's buttons; true if one is shown.
fn job_status(d: &mut Dashboard, k: &mut Kit, id: &str, y: f32) -> bool {
    let Some(j) = d.jobs.get(id) else {
        return false;
    };
    let (label, fraction) = (j.label.clone(), j.fraction);
    let bw = Btn::Small.w();
    k.progress(
        RIGHT - bw - 8.0 - 290.0,
        y + 11.0,
        290.0,
        25.0,
        fraction,
        &label,
    );
    if k.button(
        &format!("cancel-{id}"),
        Btn::Small,
        "Cancel",
        11.0,
        RIGHT - bw,
        y + 11.0,
        true,
        "Stop this job",
    ) {
        d.cancel_job(id);
    }
    true
}

fn installed_row(
    d: &mut Dashboard,
    k: &mut Kit,
    ctx: &egui::Context,
    v: &InstalledVersion,
    y: f32,
) {
    let ok = d.demo || paths::has_echo_install(&v.root);
    let sub = if ok {
        v.root.clone()
    } else {
        format!("{}  --  files missing", v.root)
    };
    let mut bx = row_base(
        k,
        y,
        &v.name,
        &sub,
        if ok { style::TEXT_DIM } else { theme::MARK_BAD },
    );
    let selected = d.state.selected_version().is_some_and(|s| s.id == v.id);
    if selected {
        bx += frame::badge(k, bx, y + 5.0, "Selected", true) + 6.0;
    }
    if v.external {
        frame::badge(k, bx, y + 5.0, "Existing folder", false);
    }
    if job_status(d, k, &v.id, y) {
        return;
    }
    let bw = Btn::Small.w();
    let menu = [
        "Update",
        "Verify files",
        "Open folder",
        if v.external { "Forget" } else { "Remove" },
    ];
    match k.menu_button(
        &format!("menu-{}", v.id),
        "Manage",
        &menu,
        RIGHT - bw,
        y + 11.0,
        bw,
        "Update, verify, open or remove",
    ) {
        Some(0) if ok => update(d, ctx, v.clone()),
        Some(1) if ok => verify(d, ctx, v.clone()),
        Some(2) => {
            if let Err(e) = platform::open_folder(&paths::bin_path(&v.root)) {
                d.dialogs.error(
                    "Couldn't open folder",
                    &format!("{e:#}"),
                    Default::default(),
                );
            }
        }
        Some(3) => {
            d.pending_remove = Some(v.id.clone());
            let (title, msg) = if v.external {
                (
                    "Forget",
                    format!(
                        "Remove {} from the launcher?\n\nThe folder {} stays on disk.",
                        v.name, v.root
                    ),
                )
            } else {
                (
                    "Remove",
                    format!("Delete {}?\n\nThis removes {} from disk.", v.name, v.root),
                )
            };
            d.dialogs.confirm(REMOVE_KEY, title, &msg, DlgIcon::Warning);
        }
        _ => {}
    }
    if k.button(
        &format!("play-{}", v.id),
        Btn::Small,
        "Play",
        11.0,
        RIGHT - 2.0 * bw - 8.0,
        y + 11.0,
        ok,
        "Select this version and go to Play",
    ) {
        d.state.selected = Some(v.id.clone());
        d.save();
        d.page = Page::Play;
        d.play_platform = Platform::Pc;
    }
}

fn available_row(d: &mut Dashboard, k: &mut Kit, ctx: &egui::Context, e: &VersionEntry, y: f32) {
    let mut sub = e.notes.clone();
    if let Some(s) = e.size {
        sub = format!("{:.1} GB  ·  {sub}", s as f64 / 1e9);
    }
    let bx = row_base(k, y, &e.name, &sub, style::TEXT_DIM);
    if !e.channel.is_empty() {
        frame::badge(k, bx, y + 5.0, &e.channel, e.channel == "stable");
    }
    if job_status(d, k, &e.id, y) {
        return;
    }
    let installed = d
        .state
        .versions
        .iter()
        .any(|v| v.catalog_id.as_deref() == Some(&e.id));
    if installed {
        let w = frame::badge_width(k, "Installed");
        frame::badge(k, RIGHT - w, y + 13.0, "Installed", true);
    } else {
        let tip = format!(
            "Install into {}",
            versions::root_for(&d.state.library, &e.id)
        );
        if k.button(
            &format!("inst-{}", e.id),
            Btn::Small,
            "Install",
            11.0,
            RIGHT - Btn::Small.w(),
            y + 11.0,
            true,
            &tip,
        ) {
            install(d, ctx, e.clone());
        }
    }
}

fn quest(kit: &mut Kit) -> Option<Open> {
    header(kit, "Echo VR on Quest", HEADER_Y);
    para(
        kit,
        "The headset holds one version. Install or update it over USB;\nthe Play page shows what is installed.",
        150.0,
        14.0,
        style::TEXT,
    );
    let mut open = None;
    let gap = 12.0;
    let x0 = CX - (2.0 * Btn::Middle.w() + gap) / 2.0;
    if kit.button(
        "vq-install",
        Btn::Middle,
        "Install on Quest",
        14.0,
        x0,
        206.0,
        true,
        "Opens the Quest install wizard",
    ) {
        open = Some(Open::QuestInstall);
    }
    if kit.button(
        "vq-update",
        Btn::Middle,
        "Update on Quest",
        14.0,
        x0 + Btn::Middle.w() + gap,
        206.0,
        true,
        "Opens the Quest update wizard",
    ) {
        open = Some(Open::QuestUpdate);
    }
    open
}
