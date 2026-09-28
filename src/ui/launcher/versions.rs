//! Versions page: installed versions and the catalogue as a list of cards.

use super::{Dashboard, JobResult, Page, BESIDE_TITLE, CW, X0};
use crate::core::error::UiError;
use crate::core::launcher::catalog::{Platform, VersionEntry};
use crate::core::launcher::store::InstalledVersion;
use crate::core::launcher::versions;
use crate::core::{paths, platform};
use crate::ui::dialogs::Icon as DlgIcon;
use crate::ui::kit::Kit;
use crate::ui::launcher::Open;
use crate::ui::parts;
use crate::ui::style::{self, Icon, MenuItem, Variant};

const REMOVE_KEY: &str = "remove-version";
const REPAIR_KEY: &str = "repair-version";
const LIST_Y: f32 = 112.0;
const LIST_H: f32 = 530.0;
const ROW_H: f32 = 62.0;
const ROW_GAP: f32 = 8.0;

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
        &format!("Updating {}", v.name),
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
        &format!("Verifying {}", v.name),
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
        &format!("Installing {}", e.name),
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

/// Adds the Echo VR install in the Meta (Oculus) library, if there is one.
pub(super) fn find_meta(d: &mut Dashboard) {
    let Some(base) = crate::core::platform::oculus_base_path() else {
        d.dialogs.error(
            "Meta install not found",
            "Could not find the Meta Quest (Oculus) app on this PC.",
            Default::default(),
        );
        return;
    };
    let sep = if base.ends_with(['\\', '/']) { "" } else { "/" };
    let root = paths::normalize(&format!("{base}{sep}Software/Software"));
    if !paths::has_echo_install(&root) {
        d.dialogs.info(
            "Echo VR not in your Meta library",
            &format!(
                "Echo VR is not installed in your Meta library ({root}).\n\n\
                 If you own it, install it from the Meta Store and launch it once. \
                 Otherwise install a version from the ▾ menu next to PLAY."
            ),
        );
        return;
    }
    match d
        .state
        .add_external(&root, Some("Echo VR (Meta library)".into()))
    {
        Some(id) => {
            d.state.selected = Some(id);
            d.save();
        }
        None => d
            .dialogs
            .info("Already added", "Your Meta install is already in the list."),
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

    let sel = if d.versions_platform == Platform::Pc {
        0
    } else {
        1
    };
    if let Some(i) = kit.segmented(
        "versions-platform",
        &["PC", "Quest"],
        sel,
        BESIDE_TITLE,
        61.0,
        190.0,
        30.0,
    ) {
        d.versions_platform = if i == 0 {
            Platform::Pc
        } else {
            Platform::Quest
        };
    }
    if d.versions_platform == Platform::Quest {
        return quest(kit);
    }

    let installed: Vec<InstalledVersion> = d.state.versions.clone();
    let available: Vec<VersionEntry> = d
        .catalog
        .as_ref()
        .map(|c| d.state.not_installed(c).into_iter().cloned().collect())
        .unwrap_or_default();
    let content_h =
        60.0 + (installed.len().max(1) + available.len().max(1)) as f32 * (ROW_H + ROW_GAP);
    kit.scroll(X0, LIST_Y, CW, LIST_H, content_h, &mut d.versions_scroll);
    let scroll = d.versions_scroll;
    kit.clipped(X0 - 4.0, LIST_Y, CW + 8.0, LIST_H, |k| {
        let mut y = LIST_Y - scroll;
        k.caps(X0, y + 4.0, "Installed", style::TEXT_MUTED);
        y += 26.0;
        if installed.is_empty() {
            k.card(X0, y, CW, ROW_H);
            k.text(
                X0 + 24.0,
                y + 23.0,
                "Nothing installed yet. Install a version below, or add a folder you already have.",
                style::body(14.0),
                style::TEXT_DIM,
            );
            y += ROW_H + ROW_GAP;
        }
        for v in &installed {
            installed_row(d, k, ctx, v, y);
            y += ROW_H + ROW_GAP;
        }
        y += 8.0;
        let source = match &d.catalog {
            None => "Available  ·  loading...",
            Some(c) if c.builtin => "Available  ·  built-in list",
            _ => "Available",
        };
        k.caps(X0, y + 4.0, source, style::TEXT_MUTED);
        y += 26.0;
        if available.is_empty() && d.catalog.is_some() {
            k.card(X0, y, CW, ROW_H);
            k.text(
                X0 + 24.0,
                y + 23.0,
                "All available versions are installed.",
                style::body(14.0),
                style::TEXT_DIM,
            );
        }
        for e in &available {
            available_row(d, k, ctx, e, y);
            y += ROW_H + ROW_GAP;
        }
    });

    // Footer.
    let fy = 658.0;
    if kit
        .flat_button(
            "add-existing",
            Variant::Secondary,
            Some(Icon::Folder),
            "Add existing folder",
            X0,
            fy,
            250.0,
            style::MID,
            true,
            "Use an Echo VR install that is already on this PC",
        )
        .clicked
    {
        add_existing(d);
    }
    let mut x = X0 + 262.0;
    if cfg!(windows) || d.demo {
        if kit
            .flat_button(
                "find-meta",
                Variant::Secondary,
                None,
                "Find Meta install",
                x,
                fy,
                230.0,
                style::MID,
                true,
                "Add Echo VR from your Meta (Oculus) library",
            )
            .clicked
        {
            find_meta(d);
        }
        x += 242.0;
    }
    if kit
        .flat_button(
            "refresh",
            Variant::Secondary,
            Some(Icon::Refresh),
            "Refresh",
            x,
            fy,
            150.0,
            style::MID,
            !d.catalog_loading,
            "Reload the list of available versions",
        )
        .clicked
    {
        d.refresh_catalog(ctx);
    }
    None
}

/// A row's box with the name (and badges after it) over a detail line; returns where
/// badges may start.
fn row_base(k: &Kit, y: f32, name: &str, sub: &str, sub_color: egui::Color32) -> f32 {
    k.card(X0, y, CW, ROW_H);
    let font = style::display(13.0);
    k.text_fit(X0 + 24.0, y + 12.0, 480.0, name, font.clone(), style::TEXT);
    k.text_fit(
        X0 + 24.0,
        y + 36.0,
        640.0,
        sub,
        style::body(12.0),
        sub_color,
    );
    X0 + 24.0 + k.text_width(name, font).min(480.0) + 12.0
}

/// Right edge of a row's buttons, and their vertical position in the row.
const RIGHT: f32 = X0 + CW - 16.0;
const BTN_DY: f32 = (ROW_H - style::SMALL) / 2.0;

/// A running job in place of a row's actions; true if one is shown.
fn job_status(d: &mut Dashboard, k: &mut Kit, id: &str, y: f32) -> bool {
    let Some(j) = d.jobs.get(id) else {
        return false;
    };
    let (label, fraction) = (j.label.clone(), j.fraction);
    k.progress(
        RIGHT - 110.0 - 10.0 - 320.0,
        y + BTN_DY - 4.0,
        320.0,
        fraction,
        &label,
    );
    if k.flat_button(
        &format!("cancel-{id}"),
        Variant::Ghost,
        None,
        "Cancel",
        RIGHT - 110.0,
        y + BTN_DY,
        110.0,
        style::SMALL,
        true,
        "Stop this job",
    )
    .clicked
    {
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
        format!("{}  —  files missing", v.root)
    };
    let mut bx = row_base(
        k,
        y,
        &v.name,
        &sub,
        if ok { style::TEXT_MUTED } else { style::DANGER },
    );
    let selected = d.state.selected.as_deref() == Some(v.id.as_str());
    if selected {
        bx += k.pill(bx, y + 10.0, "Selected", style::OK, false) + 6.0;
    }
    if v.external {
        k.pill(bx, y + 10.0, "Existing folder", style::CHIP_OFF, false);
    }
    if job_status(d, k, &v.id, y) {
        return;
    }
    let menu = [
        MenuItem::row("Update")
            .tip("Download any changed game files")
            .item(),
        MenuItem::row("Verify files")
            .tip("Check every game file against the update manifest")
            .item(),
        MenuItem::row("Open folder").item(),
        MenuItem::Divider,
        MenuItem::row(if v.external { "Forget" } else { "Remove" })
            .tip(if v.external {
                "Remove from the launcher; the folder stays on disk"
            } else {
                "Delete this version from disk"
            })
            .item(),
    ];
    match k.menu_button(
        &format!("menu-{}", v.id),
        "Manage",
        &menu,
        RIGHT - 130.0,
        y + BTN_DY,
        130.0,
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
        Some(4) => {
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
    if k.flat_button(
        &format!("play-{}", v.id),
        Variant::Secondary,
        None,
        "Play",
        RIGHT - 130.0 - 10.0 - 110.0,
        y + BTN_DY,
        110.0,
        style::SMALL,
        ok,
        "Select this version and go to Play",
    )
    .clicked
    {
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
    let bx = row_base(k, y, &e.name, &sub, style::TEXT_MUTED);
    if !e.channel.is_empty() {
        let color = if e.channel == "stable" {
            style::OK
        } else {
            style::CHIP_OFF
        };
        k.pill(bx, y + 10.0, &e.channel, color, false);
    }
    if job_status(d, k, &e.id, y) {
        return;
    }
    let tip = format!(
        "Install into {}",
        versions::root_for(&d.state.library, &e.id)
    );
    if k.flat_button(
        &format!("inst-{}", e.id),
        Variant::Primary,
        None,
        "Install",
        RIGHT - 130.0,
        y + BTN_DY,
        130.0,
        style::SMALL,
        !d.any_job(),
        &tip,
    )
    .clicked
    {
        d.state.selected = Some(e.id.clone());
        install(d, ctx, e.clone());
    }
}

fn quest(kit: &mut Kit) -> Option<Open> {
    let (w, h) = (640.0, 240.0);
    let x = X0 + (CW - w) / 2.0;
    let y = 180.0;
    kit.card(x, y, w, h);
    let top = kit.rect(x + w / 2.0 - 20.0, y + 30.0, 40.0, 40.0).min;
    style::icon_at(kit.ui.painter(), Icon::Headset, top, 40.0, style::ACCENT);
    kit.text_center(
        x,
        y + 84.0,
        w,
        28.0,
        "Echo VR on Quest",
        style::display(18.0),
        style::TEXT,
        None,
    );
    kit.text_center(
        x + 40.0,
        y + 116.0,
        w - 80.0,
        40.0,
        "The headset holds one version. Install or update it over USB; the Play page shows what is installed.",
        style::body(14.0),
        style::TEXT_DIM,
        Some(w - 80.0),
    );
    let mut open = None;
    let bw = 220.0;
    if kit
        .flat_button(
            "vq-install",
            Variant::Primary,
            None,
            "Install on Quest",
            x + w / 2.0 - bw - 8.0,
            y + 176.0,
            bw,
            style::MID,
            true,
            "Install Echo VR on your Quest over USB",
        )
        .clicked
    {
        open = Some(Open::QuestInstall);
    }
    if kit
        .flat_button(
            "vq-update",
            Variant::Secondary,
            None,
            "Update on Quest",
            x + w / 2.0 + 8.0,
            y + 176.0,
            bw,
            style::MID,
            true,
            "Copy the latest game files to your Quest",
        )
        .clicked
    {
        open = Some(Open::QuestUpdate);
    }
    open
}
