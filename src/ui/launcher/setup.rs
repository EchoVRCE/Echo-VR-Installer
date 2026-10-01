//! What the installer wizards did, inline: the first-run setup (own Echo VR or not, how
//! you play), the licence patch (Discord or a link) and the SteamVR (Revive) setup.

use std::sync::mpsc::sync_channel;

use super::versions::job_err;
use super::{Dashboard, JobKind, JobResult, Msg, H, W};
use crate::core::error::UiError;
use crate::core::launcher::launch;
use crate::core::launcher::patch::{self, FetchError, Source};
use crate::core::launcher::quest::{self as quest_core, ApkSource, JobError, UpdateOutcome};
use crate::core::launcher::store::{InstalledVersion, Runtime};
use crate::core::launcher::versions::Step;
use crate::core::{download, elevation, oauth, paths, platform, revive};
use crate::ui::design::{self, dz};
use crate::ui::kit::Kit;
use crate::ui::widgets::{Tone, BTN_H};

pub(super) const REVIVE_JOB: &str = "revive";
pub(super) const CONSENT_KEY: &str = "admin-consent";
pub(super) const JOIN_KEY: &str = "join-server";
pub(super) const QUEST_JOB: &str = "quest";
pub(super) const QUEST_INSTALL_KEY: &str = "quest-install";
pub(super) const QUEST_REINSTALL_KEY: &str = "quest-reinstall";

/// Something to carry on with once the first-run setup is answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Resume {
    QuestInstall,
}

/// A card over the whole window.
pub(super) enum Overlay {
    /// "Welcome To Echo VR": step 0 asks about the licence, step 1 how you play.
    Setup { step: u8 },
    /// A patch from a link the user already has.
    PatchLink { target: LinkFor, url: String },
}

/// What a patch link is for.
#[derive(Clone)]
pub(super) enum LinkFor {
    /// The licence patch for this PC version.
    Pc(String),
    /// A patched APK, installed on the Quest.
    Quest,
}

// ---- jobs ----

/// Fetches the personal licence patch and puts it into version `id`.
pub(super) fn patch(d: &mut Dashboard, ctx: &egui::Context, id: &str, source: Source) {
    let Some(v) = d.state.version(id).cloned() else {
        return;
    };
    let first = match source {
        Source::Discord => "Opening Discord in your browser...",
        Source::Url(_) => "Downloading your patch...",
    };
    d.start_job(
        ctx,
        JobKind::Patch,
        id,
        &format!("Patching {}", v.name),
        first,
        move |cancel, on| {
            // A patch fetched earlier this session is personal too: reuse it.
            let staged = patch::staged();
            let dll = if source == Source::Discord && staged.is_file() {
                staged
            } else {
                match patch::fetch(&source, cancel, on) {
                    Ok(p) => p,
                    Err(FetchError::OAuth(e)) => return JobResult::OAuthFailed(e),
                    Err(FetchError::Other(e)) => return job_err(e, "Licence Patch Failed"),
                }
            };
            on(Step::Status("Applying the patch...".into()));
            match patch::apply(&v.root, &dll) {
                Ok(()) => JobResult::Patched,
                Err(e) => job_err(e, "Couldn't write patch"),
            }
        },
    );
}

/// Takes the licence patch off version `id` again.
pub(super) fn unpatch(d: &mut Dashboard, id: &str) {
    let Some(v) = d.state.version(id).cloned() else {
        return;
    };
    match patch::remove(&v.root) {
        Ok(()) => {
            if let Some(x) = d.state.versions.iter_mut().find(|x| x.id == id) {
                x.patched = false;
            }
            d.save();
            d.notify("The licence patch is removed: the original pnsovr.dll is back");
        }
        Err(e) => d.dialogs.error(
            "Couldn't remove patch",
            &format!("{e:#}"),
            Default::default(),
        ),
    }
}

/// Downloads and installs Revive (asking for administrator rights), then the artwork.
pub(super) fn revive(d: &mut Dashboard, ctx: &egui::Context) {
    let tx = d.worker.tx(ctx);
    let artwork = d.state.revive_artwork;
    d.start_job(
        ctx,
        JobKind::Revive,
        REVIVE_JOB,
        "Setting up SteamVR",
        "Downloading Revive...",
        move |cancel, on| {
            let mut consent = || {
                let (s, r) = sync_channel(1);
                tx.send(Msg::Consent(s));
                r.recv().unwrap_or(false)
            };
            let installer = match revive::download_installer(cancel, &mut |p| {
                if let download::Progress::Percent(v) = p {
                    on(Step::Percent(v));
                }
            }) {
                Ok(p) => p,
                Err(e) => return job_err(e, "SteamVR Setup Failed"),
            };
            on(Step::Status("Installing Revive...".into()));
            if let Err(e) = elevation::run_revive_installer(&installer, &mut consent) {
                return job_err(
                    e.context("Installing Revive failed"),
                    "SteamVR Setup Failed",
                );
            }
            on(Step::Status("Waiting for Revive...".into()));
            if revive::wait_for_revive_dir(std::time::Duration::from_secs(8)).is_none() {
                return JobResult::Failed(Some(UiError::new(
                    "SteamVR Setup Failed",
                    "Revive does not appear to be installed (was the installer cancelled?).",
                )));
            }
            if artwork {
                on(Step::Status("Installing the game artwork...".into()));
                if let Err(e) = elevation::install_artwork(&mut consent) {
                    return job_err(
                        e.context("Installing the artwork failed"),
                        "SteamVR Setup Failed",
                    );
                }
            }
            JobResult::ReviveReady
        },
    );
}

/// A desktop shortcut to version `id` (through Revive when that is the launch mode).
pub(super) fn shortcut(d: &mut Dashboard, id: &str) {
    let Some(v) = d.state.version(id).cloned() else {
        return;
    };
    let exe = paths::exe_path(&v.root);
    let revive = match d.state.profile.runtime {
        Runtime::Revive if !d.demo => revive::find_revive_dir(),
        _ => None,
    };
    // The launch options from Settings go into the shortcut too.
    let args = launch::join_args(&launch::game_args(&d.state.profile, None));
    let result = match (d.state.profile.runtime, revive) {
        (Runtime::Revive, Some(dir)) => revive::create_injector_shortcut(&dir, &exe),
        _ => platform::create_shortcut(
            "Echo VR",
            &exe,
            (!args.is_empty()).then_some(args.as_str()),
            Some(&paths::bin_path(&v.root)),
            Some(&exe),
        ),
    };
    match result {
        Ok(()) => d.notify("The desktop shortcut is ready"),
        Err(e) => d.dialogs.error(
            "Couldn't create shortcut",
            &format!("{e:#}"),
            Default::default(),
        ),
    }
}

/// The licence patch: PATCH on PLAY, the Manage menu's patch entries, patched Quest
/// builds and the welcome's licence question. Off until it comes back in another form;
/// meanwhile everyone is treated as owning Echo VR.
pub(super) const LICENCE_PATCH: bool = false;

/// The welcome's first question: the licence, or how you play.
const FIRST_STEP: u8 = if LICENCE_PATCH { 0 } else { 1 };

/// The first-run welcome, at its first question.
pub(super) fn welcome() -> Overlay {
    Overlay::Setup { step: FIRST_STEP }
}

/// Version `v` needs the licence patch before PLAY.
pub(super) fn needs_patch(d: &Dashboard, v: &InstalledVersion) -> bool {
    LICENCE_PATCH && d.state.owner == Some(false) && !v.patched
}

/// The Quest APK to install: stock for owners, a personal patched one for new players.
pub(super) fn quest_source(d: &Dashboard) -> ApkSource {
    if LICENCE_PATCH && d.state.owner == Some(false) {
        ApkSource::Discord
    } else {
        ApkSource::Stock
    }
}

/// Asks before replacing Echo VR on the headset (the setup comes first if unanswered).
pub(super) fn ask_quest_install(d: &mut Dashboard) {
    if LICENCE_PATCH && d.state.owner.is_none() {
        d.overlay = Some(welcome());
        d.after_setup = Some(Resume::QuestInstall);
        return;
    }
    d.dialogs.confirm(
        QUEST_INSTALL_KEY,
        "Install Echo VR",
        "Installing replaces Echo VR on your Quest.\nThe installed app and its local data will be removed first.\n\nContinue?",
        crate::ui::dialogs::Icon::Question,
    );
}

pub(super) fn quest_install(d: &mut Dashboard, ctx: &egui::Context, source: ApkSource) {
    let first = match source {
        ApkSource::Discord => "Opening Discord in your browser...",
        _ => "Checking for the latest version...",
    };
    d.start_job(
        ctx,
        JobKind::QuestInstall,
        QUEST_JOB,
        "Installing Echo VR on your Quest",
        first,
        move |cancel, on| match quest_core::install(&source, cancel, on) {
            Ok(()) => JobResult::QuestInstalled,
            Err(JobError::OAuth(e)) => JobResult::OAuthFailed(e),
            Err(JobError::Other(e)) => job_err(e, "Installation Failed"),
        },
    );
}

pub(super) fn quest_update(d: &mut Dashboard, ctx: &egui::Context) {
    d.start_job(
        ctx,
        JobKind::QuestUpdate,
        QUEST_JOB,
        "Updating Echo VR on your Quest",
        "Checking your Quest...",
        move |cancel, on| match quest_core::update(cancel, on) {
            Ok(UpdateOutcome::Updated) => JobResult::QuestUpdated,
            Ok(UpdateOutcome::NeedsReinstall(detail)) => JobResult::QuestNeedsReinstall(detail),
            Err(e) => job_err(e, "Update Failed"),
        },
    );
}

// ---- overlay cards ----

pub(super) fn draw_overlay(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    if d.overlay.is_none() {
        return;
    }
    let blocked = kit.blocked;
    kit.modal("overlay", blocked, |k| match &d.overlay {
        Some(Overlay::Setup { step }) => {
            let step = *step;
            setup_card(d, k, ctx, step)
        }
        Some(Overlay::PatchLink { .. }) => link_card(d, k, ctx),
        None => {}
    });
}

pub(super) const OWN_NOTE: &str =
    "You play with your own licence from the Meta store. The licence patch stays optional.";
pub(super) const NEW_NOTE: &str =
    "No licence yet? You get a personal licence patch through Discord before your first match.";

pub(super) fn runtime_note(r: Runtime) -> &'static str {
    match r {
        Runtime::MetaLink => "Quest over Link or Air Link, or a Rift, with the Meta Quest app.",
        Runtime::VirtualDesktop => "Quest over Virtual Desktop; start its streamer first.",
        Runtime::Revive => "Any SteamVR headset. The launcher sets up Revive for you.",
        Runtime::Flat => "No headset: play or spectate on the monitor.",
    }
}

/// A runtime's name on its tile (the welcome's, Settings').
pub(super) fn runtime_title(r: Runtime) -> &'static str {
    match r {
        Runtime::Revive => "SteamVR (Revive)",
        Runtime::Flat => "Flat (no headset)",
        _ => runtime_label(r),
    }
}

pub(super) fn runtime_label(r: Runtime) -> &'static str {
    match r {
        Runtime::MetaLink => "Meta Link",
        Runtime::VirtualDesktop => "Virtual Desktop",
        Runtime::Revive => "SteamVR",
        Runtime::Flat => "Flat",
    }
}

/// A modal card with a header strip; returns its padding-inset content rect's left, top
/// and width.
fn card(k: &Kit, w: f32, h: f32, title: &str) -> (f32, f32, f32, f32) {
    let (x, y) = (
        ((W + k.ex - w) / 2.0).round(),
        ((H + k.ey - h) / 2.0).round(),
    );
    k.solid_panel(x, y, w, h);
    k.header_strip(x, y, w, dz(46.0), title);
    let pad = dz(30.0);
    (x + pad, y + dz(46.0) + pad, w - 2.0 * pad, y + h - pad)
}

/// "Welcome to Echo VR": one question per step, each answer a tile with its explanation.
fn setup_card(d: &mut Dashboard, k: &mut Kit, ctx: &egui::Context, step: u8) {
    // Both steps share one size: the four answers of step 2 set it.
    let (w, h) = (dz(1000.0), dz(515.0));
    let (x, y, cw, bottom) = card(k, w, h, "Welcome to Echo VR");
    let question = if step == 0 {
        "Do you own Echo VR on your Meta account?"
    } else {
        "How do you play Echo VR?"
    };
    let q = k.spaced_fit(
        &question.to_uppercase(),
        design::conthrax(20.0),
        design::TEXT,
        dz(2.0),
        false,
        cw,
    );
    k.put(x, y, q);
    let top = y + dz(52.0);
    let gap = dz(20.0);
    let tw = (cw - gap) / 2.0;
    if step == 0 {
        let th = dz(260.0);
        let answers = [
            (true, "I own Echo VR on Meta", OWN_NOTE),
            (false, "I'm a new player", NEW_NOTE),
        ];
        for (i, (own, label, note)) in answers.into_iter().enumerate() {
            let tx = x + i as f32 * (tw + gap);
            let on = d.state.owner == Some(own);
            if k.tile(
                &format!("setup-owner-{i}"),
                tx,
                top,
                tw,
                th,
                label,
                note,
                on,
            )
            .clicked
            {
                d.state.owner = Some(own);
                d.overlay = Some(Overlay::Setup { step: 1 });
            }
        }
    } else {
        let th = dz(120.0);
        for (i, rt) in Runtime::ALL.iter().enumerate() {
            let tx = x + (i % 2) as f32 * (tw + gap);
            let ty = top + (i / 2) as f32 * (th + gap);
            let label = runtime_title(*rt);
            let on = d.state.profile.runtime == *rt;
            let note = runtime_note(*rt);
            if k.tile(
                &format!("setup-runtime-{i}"),
                tx,
                ty,
                tw,
                th,
                label,
                note,
                on,
            )
            .clicked
            {
                d.state.profile.runtime = *rt;
                finish_setup(d);
            }
        }
    }

    // BACK, SKIP FOR NOW and the step.
    let by = bottom - BTN_H;
    let mut bx = x;
    if step > FIRST_STEP {
        let bw = k.button_width("Back", None, BTN_H).max(110.0);
        if k.button(
            "setup-back",
            bx,
            by,
            bw,
            BTN_H,
            Tone::Dark,
            None,
            "Back",
            true,
            "",
        )
        .clicked
        {
            d.overlay = Some(Overlay::Setup { step: 0 });
        }
        bx += bw + dz(24.0);
    }
    let skip_tip = "Ask later; you can change it in Settings";
    let skip_y = by + (BTN_H - dz(20.0)) / 2.0;
    if k.link("setup-skip", bx, skip_y, "Skip for now", 16.0, skip_tip)
        .clicked
        || ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        finish_setup(d);
        return;
    }
    if FIRST_STEP == 0 {
        let steps = format!("Step {} of 2", step + 1);
        let g = k.label_galley(&steps, design::din(15.0), design::GREY, f32::INFINITY);
        let gx = x + cw - g.size().x;
        k.put(gx, by + (BTN_H - g.size().y) / 2.0, g);
    }
}

fn finish_setup(d: &mut Dashboard) {
    d.state.setup_done = true;
    d.overlay = None;
    d.save();
    if d.after_setup.take() == Some(Resume::QuestInstall) && d.state.owner.is_some() {
        ask_quest_install(d);
    }
}

/// The licence patch from a link the user already has.
fn link_card(d: &mut Dashboard, k: &mut Kit, ctx: &egui::Context) {
    let busy = d.any_job();
    let Some(Overlay::PatchLink { target, url }) = &mut d.overlay else {
        return;
    };
    let quest = matches!(target, LinkFor::Quest);
    let validate = if quest {
        oauth::validate_apk_url
    } else {
        oauth::validate_dll_url
    };
    let title = if quest {
        "Patched APK from a link"
    } else {
        "Licence patch from a link"
    };
    let (w, h) = (dz(960.0), dz(310.0));
    let (x, y, cw, bottom) = card(k, w, h, title);
    let hint = "Paste the patch link you got from the Echo VR Discord (files.echovr.de).";
    let th = k.caps_text(x, y, cw, hint, 17.0, design::BODY, 0.0);
    let fy = y + th + dz(20.0);
    let pw = k.button_width("Paste", None, BTN_H).max(100.0);
    let invalid = !url.trim().is_empty() && validate(url.trim()).is_none();
    k.field(
        "patch-url",
        url,
        x,
        fy,
        cw - pw - 10.0,
        BTN_H,
        "https://files.echovr.de/...",
        invalid,
        "The link to your personal patch",
    );
    let paste_tip = "Paste a link from your clipboard";
    if k.button(
        "patch-paste",
        x + cw - pw,
        fy,
        pw,
        BTN_H,
        Tone::Dark,
        None,
        "Paste",
        true,
        paste_tip,
    )
    .clicked
    {
        if let Some(clip) = arboard::Clipboard::new()
            .ok()
            .and_then(|mut c| c.get_text().ok())
        {
            *url = clip.trim().to_string();
        }
    }
    if invalid {
        let msg = "That doesn't look like a patch link.";
        k.caps_text(x, fy + BTN_H + dz(12.0), cw, msg, 16.0, design::DANGER, 0.0);
    }
    let valid = validate(url.trim()).is_some();
    let (target, link) = (target.clone(), url.trim().to_string());
    let by = bottom - BTN_H;
    let apply = if quest { "Install" } else { "Apply patch" };
    let aw = k.button_width(apply, None, BTN_H).max(140.0);
    let cw2 = k.button_width("Cancel", None, BTN_H).max(110.0);
    let right = x + cw;
    if k.button(
        "patch-cancel",
        right - cw2,
        by,
        cw2,
        BTN_H,
        Tone::Dark,
        None,
        "Cancel",
        true,
        "",
    )
    .clicked
        || ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        d.overlay = None;
        return;
    }
    let apply_tip = if quest {
        "Download the patched APK and install it on your Quest"
    } else {
        "Download the patch and put it into this version"
    };
    let ax = right - cw2 - 8.0 - aw;
    if k.button(
        "patch-apply",
        ax,
        by,
        aw,
        BTN_H,
        Tone::Go,
        None,
        apply,
        valid && !busy,
        apply_tip,
    )
    .clicked
    {
        d.overlay = None;
        match target {
            LinkFor::Pc(id) => patch(d, ctx, &id, Source::Url(link)),
            LinkFor::Quest => quest_install(d, ctx, ApkSource::Url(link)),
        }
    }
}
