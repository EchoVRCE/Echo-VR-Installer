//! Versions page: installed versions (update/verify/folder/remove) and the catalogue.

use super::{header, Dashboard, JobResult, PAGE_W};
use crate::core::error::UiError;
use crate::core::launcher::catalog::{Platform, VersionEntry};
use crate::core::launcher::store::InstalledVersion;
use crate::core::launcher::versions;
use crate::core::{paths, platform};
use crate::ui::dialogs::Icon;
use crate::ui::kit::{Btn, Kit};
use crate::ui::launcher::Open;
use crate::ui::parts;
use crate::ui::theme;

const REMOVE_KEY: &str = "remove-version";
const REPAIR_KEY: &str = "repair-version";

pub(super) fn ask_repair(d: &mut Dashboard, id: &str, msg: &str) {
    d.pending_repair = Some(id.to_string());
    d.dialogs.confirm(REPAIR_KEY, "Verify", msg, Icon::Warning);
}

fn job_err(e: anyhow::Error, title: &str) -> JobResult {
    if crate::core::http::is_cancelled(&e) {
        JobResult::Failed(None)
    } else {
        JobResult::Failed(Some(UiError::from_anyhow(&e, title)))
    }
}

fn update(d: &mut Dashboard, ctx: &egui::Context, v: InstalledVersion) {
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
        "Verifying...",
        move |_, on| match versions::verify(&v, on) {
            Ok(bad) => JobResult::Verified(bad),
            Err(e) => job_err(e, "Verify Failed"),
        },
    );
}

fn install(d: &mut Dashboard, ctx: &egui::Context, e: VersionEntry) {
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

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    // Confirmation answers.
    if let Some(a) = d.dialogs.take(REMOVE_KEY) {
        let id = d.pending_remove.take();
        if let (true, Some(id)) = (a.is_yes(), id) {
            remove(d, &id);
        }
    }
    if let Some(a) = d.dialogs.take(REPAIR_KEY) {
        let id = d.pending_repair.take();
        if let (true, Some(v)) = (a.is_yes(), id.and_then(|id| d.state.version(&id).cloned())) {
            update(d, ctx, v);
        }
    }

    header(kit, "Versions");
    let mid = PAGE_W / 2.0;
    if kit.chip(
        "v-pc",
        "PC",
        12.0,
        mid - 130.0,
        52.0,
        120.0,
        26.0,
        d.versions_platform == Platform::Pc,
        "Echo VR versions on this PC",
    ) {
        d.versions_platform = Platform::Pc;
    }
    if kit.chip(
        "v-quest",
        "Quest",
        12.0,
        mid + 10.0,
        52.0,
        120.0,
        26.0,
        d.versions_platform == Platform::Quest,
        "Echo VR on your Quest",
    ) {
        d.versions_platform = Platform::Quest;
    }
    if d.versions_platform == Platform::Quest {
        return quest(kit);
    }

    let (lx, ly, lw, lh) = (0.0, 88.0, PAGE_W, 300.0);
    kit.round_box(
        lx,
        ly,
        lw,
        lh,
        10.0,
        theme::rgba(30, 20, 40, 150),
        Some(theme::BOX_BORDER),
    );
    let installed: Vec<InstalledVersion> = d.state.versions.clone();
    let available: Vec<VersionEntry> = d
        .catalog
        .as_ref()
        .map(|c| c.pc().cloned().collect())
        .unwrap_or_default();
    let row_h = 50.0;
    let content_h =
        26.0 + installed.len().max(1) as f32 * row_h + 30.0 + available.len().max(1) as f32 * row_h;
    kit.scroll(lx, ly, lw, lh, content_h + 10.0, &mut d.versions_scroll);
    let scroll = d.versions_scroll;
    let mut open = None;
    kit.clipped(lx + 2.0, ly + 2.0, lw - 4.0, lh - 4.0, |k| {
        let mut y = ly + 6.0 - scroll;
        k.text_left(
            14.0,
            y,
            20.0,
            "Installed",
            theme::arial_bold(14.0),
            theme::WHITE,
        );
        y += 26.0;
        if installed.is_empty() {
            k.text_left(
                24.0,
                y + 10.0,
                20.0,
                "Nothing installed yet -- pick a version below or add an existing folder.",
                theme::arial(13.0),
                theme::LIGHT_GRAY,
            );
            y += row_h;
        }
        for v in &installed {
            installed_row(d, k, ctx, v, y);
            y += row_h;
        }
        y += 4.0;
        let source = match &d.catalog {
            None if d.catalog_loading => "Available (loading...)",
            Some(c) if c.builtin => {
                "Available (built-in list; the online catalogue is not reachable)"
            }
            _ => "Available",
        };
        k.text_left(14.0, y, 20.0, source, theme::arial_bold(14.0), theme::WHITE);
        y += 26.0;
        for e in &available {
            available_row(d, k, ctx, e, y);
            y += row_h;
        }
    });

    // Bottom actions.
    let by = 400.0;
    let bw = Btn::Small.w();
    let gap = (PAGE_W - 4.0 * bw) / 5.0;
    let x = |i: f32| (gap + i * (bw + gap)).floor();
    if kit.button(
        "add-existing",
        Btn::Small,
        "Add existing",
        11.0,
        x(0.0),
        by,
        true,
        "Add an Echo VR folder that is already on this PC",
    ) {
        if let Some(p) = parts::choose_folder() {
            let root = paths::resolve_install_root(&p);
            if !paths::has_echo_install(&root) {
                d.dialogs.error("Echo VR not found", "That folder doesn't contain Echo VR (ready-at-dawn-echo-arena\\bin\\win10\\echovr.exe).", Default::default());
            } else if d.state.add_external(&root, None).is_none() {
                d.dialogs
                    .info("Already added", "This install is already in the list.");
            } else {
                d.save();
            }
        }
    }
    if kit.button(
        "refresh",
        Btn::Small,
        "Refresh list",
        11.0,
        x(1.0),
        by,
        !d.catalog_loading,
        "Reload the list of available versions",
    ) {
        d.refresh_catalog(ctx);
    }
    if kit.button(
        "wiz-install",
        Btn::Small,
        "Install wizard",
        11.0,
        x(2.0),
        by,
        true,
        "The classic step-by-step installer (licence patch, Revive)",
    ) {
        open = Some(Open::PcInstall);
    }
    if kit.button(
        "wiz-update",
        Btn::Small,
        "Update wizard",
        11.0,
        x(3.0),
        by,
        true,
        "The classic step-by-step updater",
    ) {
        open = Some(Open::PcUpdate);
    }
    open
}

fn job_status(d: &mut Dashboard, k: &mut Kit, id: &str, y: f32) -> bool {
    let Some(j) = d.jobs.get(id) else {
        return false;
    };
    let label = j.label.clone();
    parts::progress_label(k, &label, 12.0, PAGE_W - 470.0, y + 11.0, 300.0, 26.0);
    if k.button(
        &format!("cancel-{id}"),
        Btn::Small,
        "Cancel",
        11.0,
        PAGE_W - 160.0,
        y + 12.0,
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
    k.round_box(
        8.0,
        y,
        PAGE_W - 16.0,
        46.0,
        8.0,
        theme::rgba(255, 255, 255, 25),
        None,
    );
    let tag = if v.external {
        "  [existing folder]"
    } else {
        ""
    };
    k.text_left(
        20.0,
        y + 4.0,
        20.0,
        &format!("{}{tag}", v.name),
        theme::conthrax(13.0),
        theme::WHITE,
    );
    let ok = paths::has_echo_install(&v.root);
    let sub = if ok {
        v.root.clone()
    } else {
        format!("{}  (missing!)", v.root)
    };
    k.text_left(
        20.0,
        y + 25.0,
        16.0,
        &sub,
        theme::arial(11.0),
        if ok {
            theme::LIGHT_GRAY
        } else {
            theme::MARK_BAD
        },
    );
    if job_status(d, k, &v.id, y) {
        return;
    }
    let bw = 110.0;
    let bx = |i: f32| PAGE_W - 12.0 - (4.0 - i) * (bw + 6.0);
    let small = |k: &mut Kit, key: &str, text: &str, i: f32, enabled: bool, tip: &str| {
        k.chip(
            &format!("{key}-{}", v.id),
            text,
            11.0,
            bx(i),
            y + 10.0,
            bw,
            26.0,
            false,
            tip,
        ) && enabled
    };
    if small(
        k,
        "upd",
        "Update",
        0.0,
        ok,
        "Download the latest game files for this version",
    ) {
        update(d, ctx, v.clone());
    }
    if small(
        k,
        "ver",
        "Verify",
        1.0,
        ok,
        "Check every game file against the update manifest",
    ) {
        verify(d, ctx, v.clone());
    }
    if small(k, "dir", "Folder", 2.0, ok, "Open the game folder") {
        if let Err(e) = platform::open_folder(&paths::bin_path(&v.root)) {
            d.dialogs.error(
                "Couldn't open folder",
                &format!("{e:#}"),
                Default::default(),
            );
        }
    }
    let remove_label = if v.external { "Forget" } else { "Remove" };
    let remove_tip = if v.external {
        "Remove from the list (the folder stays on disk)"
    } else {
        "Delete this version from disk"
    };
    if small(k, "rm", remove_label, 3.0, true, remove_tip) {
        d.pending_remove = Some(v.id.clone());
        let msg = if v.external {
            format!(
                "Remove {} from the launcher?\n\nThe folder {} stays on disk.",
                v.name, v.root
            )
        } else {
            format!("Delete {}?\n\nThis removes {} from disk.", v.name, v.root)
        };
        d.dialogs
            .confirm(REMOVE_KEY, remove_label, &msg, Icon::Warning);
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

fn available_row(d: &mut Dashboard, k: &mut Kit, ctx: &egui::Context, e: &VersionEntry, y: f32) {
    k.round_box(
        8.0,
        y,
        PAGE_W - 16.0,
        46.0,
        8.0,
        theme::rgba(255, 255, 255, 25),
        None,
    );
    k.text_left(
        20.0,
        y + 4.0,
        20.0,
        &e.name,
        theme::conthrax(13.0),
        theme::WHITE,
    );
    let mut sub = e.notes.clone();
    if let Some(s) = e.size {
        sub = format!("{:.1} GB  •  {sub}", s as f64 / 1e9);
    }
    k.text_left(
        20.0,
        y + 25.0,
        16.0,
        &sub,
        theme::arial(11.0),
        theme::LIGHT_GRAY,
    );
    if job_status(d, k, &e.id, y) {
        return;
    }
    let installed = d
        .state
        .versions
        .iter()
        .any(|v| v.catalog_id.as_deref() == Some(&e.id));
    let label = if installed { "Installed" } else { "Install" };
    let tip = format!(
        "Install into {}",
        versions::root_for(&d.state.library, &e.id)
    );
    if k.chip(
        &format!("inst-{}", e.id),
        label,
        11.0,
        PAGE_W - 12.0 - 110.0,
        y + 10.0,
        110.0,
        26.0,
        installed,
        &tip,
    ) && !installed
    {
        install(d, ctx, e.clone());
    }
}

fn quest(kit: &mut Kit) -> Option<Open> {
    kit.text_center(
        0.0,
        110.0,
        PAGE_W,
        80.0,
        "Echo VR on the Quest is installed and updated with the wizards below.\nThe Play page shows which version your headset has and can start it.",
        theme::arial(15.0),
        theme::WHITE,
        Some(760.0),
    );
    let mid = PAGE_W / 2.0;
    let mut open = None;
    if kit.button(
        "vq-install",
        Btn::Big,
        "Quest Install Echo",
        18.0,
        mid - 290.0,
        220.0,
        true,
        "Download and install Echo VR on your Quest headset",
    ) {
        open = Some(Open::QuestInstall);
    }
    if kit.button(
        "vq-update",
        Btn::Big,
        "Update Echo (Quest)",
        18.0,
        mid + 8.0,
        220.0,
        true,
        "Download and install the latest Echo VR update on your Quest",
    ) {
        open = Some(Open::QuestUpdate);
    }
    open
}
