//! What can be done to versions: install, update, verify, add or remove them, and the
//! MANAGE menu of an installed one. The Install page (`install.rs`) shows them.

use super::{setup, Dashboard, JobKind, JobResult};
use crate::core::error::UiError;
use crate::core::launcher::catalog::VersionEntry;
use crate::core::launcher::store::InstalledVersion;
use crate::core::launcher::versions;
use crate::core::{paths, platform};
use crate::ui::dialogs::Icon as DlgIcon;
use crate::ui::kit::Kit;
use crate::ui::parts;
use crate::ui::widgets::MenuItem;

const REMOVE_KEY: &str = "remove-version";
const REPAIR_KEY: &str = "repair-version";

pub(super) fn ask_repair(d: &mut Dashboard, id: &str, msg: &str) {
    d.pending_repair = Some(id.to_string());
    d.dialogs
        .confirm(REPAIR_KEY, "Verify", msg, DlgIcon::Warning);
}

pub(super) fn job_err(e: anyhow::Error, title: &str) -> JobResult {
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
        JobKind::Update,
        &id,
        &format!("Updating {}", v.name),
        "Checking for updates...",
        move |cancel, on| match versions::update(&v, cancel, on) {
            Ok(()) => JobResult::Updated,
            Err(e) => {
                let title = crate::core::pc_update::error_title(&e);
                job_err(e, &title)
            }
        },
    );
}

fn verify(d: &mut Dashboard, ctx: &egui::Context, v: InstalledVersion) {
    let id = v.id.clone();
    d.start_job(
        ctx,
        JobKind::Verify,
        &id,
        &format!("Verifying {}", v.name),
        "Verifying game files...",
        move |cancel, on| match versions::verify(&v, cancel, on) {
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
        JobKind::Install,
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
                 Otherwise install it on this page."
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

/// The answers to Remove and Repair, whichever page is showing.
pub(super) fn handle_answers(d: &mut Dashboard, ctx: &egui::Context) {
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
}

/// The MANAGE ▾ button of an installed version (logical pixels), and what its rows do.
#[allow(clippy::too_many_arguments)]
pub(super) fn manage_menu(
    d: &mut Dashboard,
    k: &mut Kit,
    ctx: &egui::Context,
    v: &InstalledVersion,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    let ok = d.demo || paths::has_echo_install(&v.root);
    enum Act {
        Update,
        Verify,
        Open,
        Shortcut,
        Patch,
        PatchLink,
        Unpatch,
        Remove,
    }
    let mut menu = vec![
        (
            MenuItem::row("Update", "Download any changed game files"),
            Some(Act::Update),
        ),
        (
            MenuItem::row(
                "Verify files",
                "Check every game file against the update manifest",
            ),
            Some(Act::Verify),
        ),
        (MenuItem::row("Open folder", ""), Some(Act::Open)),
        (
            MenuItem::row(
                "Desktop shortcut",
                "A shortcut that starts this version directly",
            ),
            Some(Act::Shortcut),
        ),
        (MenuItem::Divider, None),
    ];
    if !setup::LICENCE_PATCH {
        // Hidden for now (see `setup::LICENCE_PATCH`).
    } else if v.patched {
        menu.push((
            MenuItem::row("Remove licence patch", "Put the original pnsovr.dll back"),
            Some(Act::Unpatch),
        ));
    } else {
        menu.push((
            MenuItem::row(
                "Apply licence patch",
                "Authorize with Discord to get your personal patch",
            ),
            Some(Act::Patch),
        ));
        menu.push((
            MenuItem::row(
                "Licence patch from a link…",
                "Use a patch link you already have",
            ),
            Some(Act::PatchLink),
        ));
    }
    if setup::LICENCE_PATCH {
        menu.push((MenuItem::Divider, None));
    }
    menu.push((
        MenuItem::row(
            if v.external { "Forget" } else { "Remove" },
            if v.external {
                "Remove from the launcher; the folder stays on disk"
            } else {
                "Delete this version from disk"
            },
        ),
        Some(Act::Remove),
    ));
    let (items, acts): (Vec<MenuItem>, Vec<Option<Act>>) = menu.into_iter().unzip();
    let picked = k
        .menu_button(
            &format!("menu-{}", v.id),
            "Manage",
            &items,
            x,
            y,
            w,
            h,
            "Update, verify, patch, open or remove",
        )
        .and_then(|i| acts.into_iter().nth(i).flatten());
    let busy = d.any_job();
    match picked {
        Some(Act::Update) if ok && !busy => update(d, ctx, v.clone()),
        Some(Act::Verify) if ok && !busy => verify(d, ctx, v.clone()),
        Some(Act::Shortcut) if ok => setup::shortcut(d, &v.id),
        Some(Act::Patch) if ok && !busy => {
            setup::patch(d, ctx, &v.id, crate::core::launcher::patch::Source::Discord)
        }
        Some(Act::PatchLink) if ok => {
            d.overlay = Some(setup::Overlay::PatchLink {
                target: setup::LinkFor::Pc(v.id.clone()),
                url: String::new(),
            })
        }
        Some(Act::Unpatch) => setup::unpatch(d, &v.id),
        Some(Act::Open) => {
            if let Err(e) = platform::open_folder(&paths::bin_path(&v.root)) {
                d.dialogs.error(
                    "Couldn't open folder",
                    &format!("{e:#}"),
                    Default::default(),
                );
            }
        }
        Some(Act::Remove) => {
            d.pending_remove = Some(v.id.clone());
            if v.external {
                let msg = format!(
                    "Remove {} from the launcher?\n\nThe folder {} stays on disk.",
                    v.name, v.root
                );
                d.dialogs
                    .confirm_danger(REMOVE_KEY, "Forget", &msg, "Forget");
            } else {
                let msg = format!("Delete {}?\n\nThis removes {} from disk.", v.name, v.root);
                d.dialogs
                    .confirm_danger(REMOVE_KEY, "Remove", &msg, "Delete");
            }
        }
        Some(_) if busy => d.dialogs.info(
            "Busy",
            "Another job is running. Wait until it's done, then try again.",
        ),
        Some(_) => d.dialogs.info(
            "Game files missing",
            "This version's folder is gone. Install it again, or forget it.",
        ),
        None => {}
    }
}
