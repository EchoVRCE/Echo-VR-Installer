//! Versions page: installed versions and the catalogue as a list of cards.

use super::{Dashboard, JobResult, Page, CW, X0};
use crate::core::error::UiError;
use crate::core::launcher::catalog::{Platform, VersionEntry};
use crate::core::launcher::store::InstalledVersion;
use crate::core::launcher::versions;
use crate::core::{paths, platform};
use crate::ui::dialogs::Icon as DlgIcon;
use crate::ui::kit::Kit;
use crate::ui::launcher::Open;
use crate::ui::parts;
use crate::ui::style::{self, Icon, Variant};

const REMOVE_KEY: &str = "remove-version";
const REPAIR_KEY: &str = "repair-version";
const LIST_Y: f32 = 112.0;
const LIST_H: f32 = 528.0;
const ROW_H: f32 = 76.0;
const ROW_GAP: f32 = 10.0;

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

    let sel = if d.versions_platform == Platform::Pc {
        0
    } else {
        1
    };
    if let Some(i) = kit.segmented(
        "versions-platform",
        &["PC", "Quest"],
        sel,
        X0 + 132.0,
        17.0,
        170.0,
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
    kit.text(
        X0,
        72.0,
        "Every version lives in its own folder, so you can keep several side by side.",
        style::body(14.0),
        style::TEXT_DIM,
    );

    let installed: Vec<InstalledVersion> = d.state.versions.clone();
    let available: Vec<VersionEntry> = d
        .catalog
        .as_ref()
        .map(|c| c.pc().cloned().collect())
        .unwrap_or_default();
    let content_h = 30.0
        + installed.len().max(1) as f32 * (ROW_H + ROW_GAP)
        + 40.0
        + available.len().max(1) as f32 * (ROW_H + ROW_GAP);
    kit.scroll(X0, LIST_Y, CW, LIST_H, content_h, &mut d.versions_scroll);
    let scroll = d.versions_scroll;
    kit.clipped(X0 - 4.0, LIST_Y, CW + 8.0, LIST_H, |k| {
        let mut y = LIST_Y - scroll;
        k.caps(X0, y + 4.0, "Installed", style::TEXT_MUTED);
        y += 30.0;
        if installed.is_empty() {
            k.card(X0, y, CW, ROW_H);
            k.text(
                X0 + 24.0,
                y + 29.0,
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
        y += 10.0;
        let source = match &d.catalog {
            None => "Available  ·  loading...",
            Some(c) if c.builtin => "Available  ·  built-in list",
            _ => "Available",
        };
        k.caps(X0, y + 4.0, source, style::TEXT_MUTED);
        y += 30.0;
        for e in &available {
            available_row(d, k, ctx, e, y);
            y += ROW_H + ROW_GAP;
        }
    });

    // Footer.
    let fy = 656.0;
    if kit
        .flat_button(
            "add-existing",
            Variant::Secondary,
            Some(Icon::Folder),
            "Add existing folder",
            X0,
            fy,
            210.0,
            40.0,
            true,
            "Use an Echo VR install that is already on this PC",
        )
        .clicked
    {
        add_existing(d);
    }
    if kit
        .flat_button(
            "refresh",
            Variant::Secondary,
            Some(Icon::Refresh),
            "Refresh",
            X0 + 222.0,
            fy,
            130.0,
            40.0,
            !d.catalog_loading,
            "Reload the list of available versions",
        )
        .clicked
    {
        d.refresh_catalog(ctx);
    }
    let mut open = None;
    if kit
        .flat_button(
            "classic-install",
            Variant::Ghost,
            None,
            "Classic installer",
            X0 + CW - 360.0,
            fy,
            170.0,
            40.0,
            true,
            "Step-by-step installer: licence patch and Revive setup",
        )
        .clicked
    {
        open = Some(Open::PcInstall);
    }
    if kit
        .flat_button(
            "classic-update",
            Variant::Ghost,
            None,
            "Classic updater",
            X0 + CW - 180.0,
            fy,
            180.0,
            40.0,
            true,
            "Step-by-step updater for any Echo VR folder",
        )
        .clicked
    {
        open = Some(Open::PcUpdate);
    }
    open
}

fn row_base(k: &Kit, y: f32, icon: Icon, name: &str, sub: &str, sub_color: egui::Color32) -> f32 {
    k.card(X0, y, CW, ROW_H);
    let ib = k.rect(X0 + 16.0, y + 16.0, 44.0, 44.0);
    k.ui.painter()
        .rect_filled(ib, style::R_CONTROL, style::SURFACE_HI);
    style::icon_at(
        k.ui.painter(),
        icon,
        ib.min + egui::vec2(11.0, 11.0),
        22.0,
        style::ACCENT,
    );
    k.text_fit(
        X0 + 76.0,
        y + 18.0,
        520.0,
        name,
        style::bold(16.0),
        style::TEXT,
    );
    k.text_fit(
        X0 + 76.0,
        y + 44.0,
        620.0,
        sub,
        style::body(12.0),
        sub_color,
    );
    // Where badges may start (after the name).
    X0 + 76.0 + k.text_width(name, style::bold(16.0)).min(520.0) + 10.0
}

/// A running job in place of a row's actions; true if one is shown.
fn job_status(d: &mut Dashboard, k: &mut Kit, id: &str, y: f32) -> bool {
    let Some(j) = d.jobs.get(id) else {
        return false;
    };
    let (label, fraction) = (j.label.clone(), j.fraction);
    k.progress(X0 + CW - 440.0, y + 18.0, 320.0, fraction, &label);
    if k.flat_button(
        &format!("cancel-{id}"),
        Variant::Ghost,
        None,
        "Cancel",
        X0 + CW - 104.0,
        y + 20.0,
        88.0,
        36.0,
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
        Icon::Monitor,
        &v.name,
        &sub,
        if ok { style::TEXT_MUTED } else { style::DANGER },
    );
    let selected = d.state.selected_version().is_some_and(|s| s.id == v.id);
    if selected {
        bx += k.pill(bx, y + 16.0, "Selected", style::ACCENT, false) + 6.0;
    }
    if v.external {
        k.pill(bx, y + 16.0, "Existing folder", style::TEXT_DIM, false);
    }
    if job_status(d, k, &v.id, y) {
        return;
    }
    let menu = [
        "Update",
        "Verify files",
        "Open folder",
        if v.external { "Forget" } else { "Remove" },
    ];
    match k.menu_button(
        &format!("menu-{}", v.id),
        &menu,
        X0 + CW - 16.0 - 40.0,
        y + 18.0,
        40.0,
        "More actions",
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
    if k.flat_button(
        &format!("play-{}", v.id),
        Variant::Secondary,
        Some(Icon::Play),
        "Play",
        X0 + CW - 16.0 - 40.0 - 8.0 - 104.0,
        y + 18.0,
        104.0,
        40.0,
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
    let bx = row_base(k, y, Icon::Download, &e.name, &sub, style::TEXT_MUTED);
    if !e.channel.is_empty() {
        let color = if e.channel == "stable" {
            style::OK
        } else {
            style::WARN
        };
        k.pill(bx, y + 16.0, &e.channel, color, false);
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
        let w = k.pill_width("Installed", true);
        k.pill(X0 + CW - 16.0 - w, y + 27.0, "Installed", style::OK, true);
    } else {
        let tip = format!(
            "Install into {}",
            versions::root_for(&d.state.library, &e.id)
        );
        if k.flat_button(
            &format!("inst-{}", e.id),
            Variant::Primary,
            Some(Icon::Download),
            "Install",
            X0 + CW - 16.0 - 128.0,
            y + 18.0,
            128.0,
            40.0,
            true,
            &tip,
        )
        .clicked
        {
            install(d, ctx, e.clone());
        }
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
    let bw = 200.0;
    if kit
        .flat_button(
            "vq-install",
            Variant::Primary,
            Some(Icon::Download),
            "Install on Quest",
            x + w / 2.0 - bw - 8.0,
            y + 172.0,
            bw,
            44.0,
            true,
            "Opens the Quest install wizard",
        )
        .clicked
    {
        open = Some(Open::QuestInstall);
    }
    if kit
        .flat_button(
            "vq-update",
            Variant::Secondary,
            Some(Icon::Refresh),
            "Update on Quest",
            x + w / 2.0 + 8.0,
            y + 172.0,
            bw,
            44.0,
            true,
            "Opens the Quest update wizard",
        )
        .clicked
    {
        open = Some(Open::QuestUpdate);
    }
    open
}
