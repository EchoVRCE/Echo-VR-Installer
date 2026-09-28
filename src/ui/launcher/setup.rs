//! What the installer wizards did, inline: the first-run setup (own Echo VR or not, how
//! you play), the licence patch (Discord or a link) and the SteamVR (Revive) setup.

use std::sync::mpsc::sync_channel;

use egui::{Order, Sense};

use super::versions::job_err;
use super::{Dashboard, JobResult, Msg, H, W};
use crate::core::error::UiError;
use crate::core::launcher::patch::{self, FetchError, Source};
use crate::core::launcher::quest::{self as quest_core, ApkSource, JobError, UpdateOutcome};
use crate::core::launcher::store::Runtime;
use crate::core::launcher::versions::Step;
use crate::core::{download, elevation, oauth, paths, platform, revive};
use crate::ui::kit::Kit;
use crate::ui::style::{self, Variant};

pub(super) const REVIVE_JOB: &str = "revive";
pub(super) const CONSENT_KEY: &str = "admin-consent";
pub(super) const JOIN_KEY: &str = "join-server";
pub(super) const QUEST_JOB: &str = "quest";
pub(super) const QUEST_INSTALL_KEY: &str = "quest-install";
pub(super) const QUEST_REINSTALL_KEY: &str = "quest-reinstall";

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
            d.dialogs.info(
                "Licence patch removed",
                "The original pnsovr.dll is back in place.",
            );
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
    let result = match (d.state.profile.runtime, d.revive_dir()) {
        (Runtime::Revive, Some(dir)) => revive::create_injector_shortcut(&dir, &exe),
        _ => platform::create_shortcut(
            "Echo VR",
            &exe,
            None,
            Some(&paths::bin_path(&v.root)),
            Some(&exe),
        ),
    };
    match result {
        Ok(()) => d.dialogs.info("Done", "Desktop shortcut created!"),
        Err(e) => d.dialogs.error(
            "Couldn't create shortcut",
            &format!("{e:#}"),
            Default::default(),
        ),
    }
}

/// The Quest APK to install: stock for owners, a personal patched one for new players.
pub(super) fn quest_source(d: &Dashboard) -> ApkSource {
    if d.state.owner == Some(false) {
        ApkSource::Discord
    } else {
        ApkSource::Stock
    }
}

/// Asks before replacing Echo VR on the headset (the setup comes first if unanswered).
pub(super) fn ask_quest_install(d: &mut Dashboard) {
    if d.state.owner.is_none() {
        d.overlay = Some(Overlay::Setup { step: 0 });
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
    let screen = kit.rect(0.0, 0.0, W, H);
    let assets = kit.assets;
    let mut tip = None;
    egui::Area::new(egui::Id::new("launcher-overlay"))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            // Swallow clicks so the dashboard underneath stays inert.
            ui.allocate_exact_size(screen.size(), Sense::click());
            let mut k = Kit::new(ui, assets, "overlay", kit.blocked);
            k.origin = screen.min;
            k.fill(0.0, 0.0, W, H, style::with_alpha(style::BG, 175));
            match &d.overlay {
                Some(Overlay::Setup { step }) => {
                    let step = *step;
                    setup_card(d, &mut k, step)
                }
                Some(Overlay::PatchLink { .. }) => link_card(d, &mut k, ctx),
                None => {}
            }
            tip = k.tip.take();
        });
    if tip.is_some() {
        kit.tip = tip;
    }
}

const OWN_NOTE: &str =
    "You play with your own licence from the Meta store. The licence patch stays optional.";
const NEW_NOTE: &str =
    "No licence yet? You get a personal licence patch through Discord before your first match.";

fn runtime_note(r: Runtime) -> &'static str {
    match r {
        Runtime::MetaLink => "Quest over Link or Air Link, or a Rift, with the Meta Quest app.",
        Runtime::VirtualDesktop => "Quest over Virtual Desktop; start its streamer first.",
        Runtime::Revive => "Any SteamVR headset. The launcher sets up Revive for you.",
        Runtime::Flat => "No headset: play or spectate on the monitor.",
    }
}

/// "Welcome To Echo VR", the installer's way: one question per step, big slanted answers,
/// and a tip line for the hovered one.
fn setup_card(d: &mut Dashboard, k: &mut Kit, step: u8) {
    let (w, h) = (640.0, 400.0);
    let (x, y) = (((W - w) / 2.0).floor(), ((H - h) / 2.0).floor());
    solid_card(k, x, y, w, h, "Welcome To Echo VR");
    let question = if step == 0 {
        "Do you own Echo VR on your Meta account?"
    } else {
        "How do you play Echo VR?"
    };
    let qw = k.banner_width(question, 40.0, 15.0).min(w - 48.0);
    k.banner(x + (w - qw) / 2.0, y + 56.0, qw, 40.0, question, 15.0);

    let mut tip = None;
    let row = |i: usize| y + 118.0 + i as f32 * (style::BIG + 12.0);
    if step == 0 {
        let bw = 380.0;
        let bx = x + (w - bw) / 2.0;
        let answers = [
            (true, "I own Echo on Meta", OWN_NOTE),
            (false, "I'm a new player", NEW_NOTE),
        ];
        for (i, (own, label, note)) in answers.into_iter().enumerate() {
            let r = k.choice(
                &format!("setup-owner-{i}"),
                label,
                bx,
                row(i),
                bw,
                style::BIG,
                d.state.owner == Some(own),
                "",
            );
            if r.hovered {
                tip = Some(note);
            }
            if r.clicked {
                d.state.owner = Some(own);
                d.overlay = Some(Overlay::Setup { step: 1 });
            }
        }
    } else {
        let labels = [
            "Meta Link",
            "Virtual Desktop",
            "SteamVR (Revive)",
            "Flat (no headset)",
        ];
        let bw = 280.0;
        let x0 = x + (w - 2.0 * bw - 16.0) / 2.0;
        for (i, (rt, label)) in Runtime::ALL.iter().zip(labels).enumerate() {
            let r = k.choice(
                &format!("setup-runtime-{i}"),
                label,
                x0 + (i % 2) as f32 * (bw + 16.0),
                row(i / 2),
                bw,
                style::BIG,
                d.state.profile.runtime == *rt,
                "",
            );
            if r.hovered {
                tip = Some(runtime_note(*rt));
            }
            if r.clicked {
                d.state.profile.runtime = *rt;
                finish_setup(d);
            }
        }
    }

    // The tip line, in a quiet box like the installer's tipbox.
    let ty = y + 250.0;
    k.round_box(
        x + 32.0,
        ty,
        w - 64.0,
        64.0,
        15.0,
        style::with_alpha(style::BG, 110),
        Some(style::BORDER),
    );
    k.text_center(
        x + 48.0,
        ty,
        w - 96.0,
        64.0,
        tip.unwrap_or("Hover over a choice for details. You can change both later in Settings."),
        style::body(14.0),
        style::TEXT_DIM,
        Some(w - 96.0),
    );

    let by = y + h - 24.0 - style::MID;
    let mut bx = x + 24.0;
    if step == 1 {
        if k.flat_button(
            "setup-back",
            Variant::Secondary,
            None,
            "< Back",
            bx,
            by,
            140.0,
            style::MID,
            true,
            "",
        )
        .clicked
        {
            d.overlay = Some(Overlay::Setup { step: 0 });
        }
        bx += 152.0;
    }
    if k.flat_button(
        "setup-skip",
        Variant::Ghost,
        None,
        "Skip for now",
        bx,
        by,
        200.0,
        style::MID,
        true,
        "Ask later; you can set this in Settings",
    )
    .clicked
    {
        finish_setup(d);
    }
    let steps = format!("Step {} of 2", step + 1);
    let sw = k.text_width(&steps, style::display(11.0));
    k.caps(x + w - 24.0 - sw, by + 12.0, &steps, style::TEXT_MUTED);
}

/// A titled card with an opaque backing, so the dashboard doesn't show through.
fn solid_card(k: &Kit, x: f32, y: f32, w: f32, h: f32, title: &str) {
    k.round_box(x, y, w, h, 15.0, style::SURFACE_SOLID, None);
    k.round_box(x, y, w, h, 15.0, style::with_alpha(style::BG, 200), None);
    k.titled_card(x, y, w, h, title);
}

fn finish_setup(d: &mut Dashboard) {
    d.state.setup_done = true;
    d.overlay = None;
    d.save();
}

/// The licence patch from a link the user already has.
fn link_card(d: &mut Dashboard, k: &mut Kit, ctx: &egui::Context) {
    let (w, h) = (620.0, 250.0);
    let (x, y) = (((W - w) / 2.0).floor(), ((H - h) / 2.0).floor());
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
    solid_card(k, x, y, w, h, title);
    k.text(
        x + 24.0,
        y + 52.0,
        "Paste the patch link you got from the Echo VR Discord (files.echovr.de).",
        style::body(14.0),
        style::TEXT_DIM,
    );
    let bw = 110.0;
    let invalid = !url.trim().is_empty() && validate(url.trim()).is_none();
    k.input_with(
        "patch-url",
        url,
        x + 24.0,
        y + 92.0,
        w - 48.0 - bw - 10.0,
        style::MID,
        "https://files.echovr.de/...",
        invalid,
        "The link to your personal patch",
        style::body(14.0),
    );
    if k.flat_button(
        "patch-paste",
        Variant::Secondary,
        None,
        "Paste",
        x + w - 24.0 - bw,
        y + 92.0,
        bw,
        style::MID,
        true,
        "Paste a link from your clipboard",
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
        k.text(
            x + 24.0,
            y + 138.0,
            "That doesn't look like a patch link.",
            style::body(13.0),
            style::DANGER,
        );
    }
    let valid = validate(url.trim()).is_some();
    let (target, link) = (target.clone(), url.trim().to_string());
    let by = y + h - 24.0 - style::MID;
    if k.flat_button(
        "patch-cancel",
        Variant::Ghost,
        None,
        "Cancel",
        x + 24.0,
        by + (style::MID - style::SMALL) / 2.0,
        130.0,
        style::SMALL,
        true,
        "",
    )
    .clicked
        || ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        d.overlay = None;
        return;
    }
    if k.flat_button(
        "patch-apply",
        Variant::Primary,
        None,
        if quest { "Install" } else { "Apply patch" },
        x + w - 24.0 - 220.0,
        by,
        220.0,
        style::MID,
        valid && !d.any_job(),
        if quest {
            "Download the patched APK and install it on your Quest"
        } else {
            "Download the patch and put it into this version"
        },
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
