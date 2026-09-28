//! What the installer wizards did, inline: the first-run setup (own Echo VR or not, how
//! you play), the licence patch (Discord or a link) and the SteamVR (Revive) setup.

use std::sync::mpsc::sync_channel;

use egui::{Order, Sense};

use super::versions::job_err;
use super::{Dashboard, JobResult, Msg, H, W};
use crate::core::error::UiError;
use crate::core::launcher::patch::{self, FetchError, Source};
use crate::core::launcher::store::Runtime;
use crate::core::launcher::versions::Step;
use crate::core::{download, elevation, oauth, paths, platform, revive};
use crate::ui::kit::Kit;
use crate::ui::style::{self, Variant};

pub(super) const REVIVE_JOB: &str = "revive";
pub(super) const CONSENT_KEY: &str = "admin-consent";
pub(super) const JOIN_KEY: &str = "join-server";

/// A card over the whole window.
pub(super) enum Overlay {
    Setup,
    /// Apply the licence patch from a link to this version.
    PatchLink {
        id: String,
        url: String,
    },
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
                Some(Overlay::Setup) => setup_card(d, &mut k),
                Some(Overlay::PatchLink { .. }) => link_card(d, &mut k, ctx),
                None => {}
            }
            tip = k.tip.take();
        });
    if tip.is_some() {
        kit.tip = tip;
    }
}

/// First run: own Echo VR or not, and how you play.
fn setup_card(d: &mut Dashboard, k: &mut Kit) {
    let (w, h) = (660.0, 360.0);
    let (x, y) = (((W - w) / 2.0).floor(), ((H - h) / 2.0).floor());
    solid_card(k, x, y, w, h, "Welcome to the Echo VR Launcher");
    k.text(
        x + 24.0,
        y + 52.0,
        "Two quick questions, so PLAY does the right thing. You can change both in Settings.",
        style::body(14.0),
        style::TEXT_DIM,
    );

    k.caps(
        x + 24.0,
        y + 92.0,
        "Do you own Echo VR on your Meta account?",
        style::TEXT,
    );
    let owner = match d.state.owner {
        Some(true) => 0,
        Some(false) => 1,
        None => usize::MAX,
    };
    if let Some(i) = k.segmented(
        "setup-owner",
        &["I own it", "I'm a new player"],
        owner,
        x + 24.0,
        y + 114.0,
        w - 48.0,
        32.0,
    ) {
        d.state.owner = Some(i == 0);
    }
    let owner_note = match d.state.owner {
        Some(true) => "You play with your own licence. The licence patch stays optional.",
        Some(false) => {
            "You'll get a personal licence patch through Discord before your first match."
        }
        None => "",
    };
    k.text(
        x + 24.0,
        y + 154.0,
        owner_note,
        style::body(13.0),
        style::TEXT_MUTED,
    );

    k.caps(x + 24.0, y + 196.0, "How do you play?", style::TEXT);
    let labels = ["Meta Link", "Virtual Desktop", "SteamVR", "Flat"];
    let sel = Runtime::ALL
        .iter()
        .position(|r| *r == d.state.profile.runtime)
        .unwrap_or(0);
    if let Some(i) = k.segmented(
        "setup-runtime",
        &labels,
        sel,
        x + 24.0,
        y + 218.0,
        w - 48.0,
        32.0,
    ) {
        d.state.profile.runtime = Runtime::ALL[i];
    }
    let runtime_note = match d.state.profile.runtime {
        Runtime::MetaLink => "Quest over Link or Air Link, or a Rift, with the Meta Quest app.",
        Runtime::VirtualDesktop => "Quest over Virtual Desktop; start its streamer first.",
        Runtime::Revive => "Any SteamVR headset. The launcher sets up Revive for you.",
        Runtime::Flat => "No headset: play or spectate on the monitor.",
    };
    k.text(
        x + 24.0,
        y + 258.0,
        runtime_note,
        style::body(13.0),
        style::TEXT_MUTED,
    );

    let by = y + h - 24.0 - style::MID;
    if k.flat_button(
        "setup-skip",
        Variant::Ghost,
        None,
        "Skip for now",
        x + 24.0,
        by + (style::MID - style::SMALL) / 2.0,
        160.0,
        style::SMALL,
        true,
        "Ask later; you can set this in Settings",
    )
    .clicked
    {
        finish_setup(d);
    }
    if k.flat_button(
        "setup-done",
        Variant::Primary,
        None,
        "Let's go",
        x + w - 24.0 - 200.0,
        by,
        200.0,
        style::MID,
        d.state.owner.is_some(),
        "Save and start",
    )
    .clicked
    {
        finish_setup(d);
    }
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
    solid_card(k, x, y, w, h, "Licence patch from a link");
    k.text(
        x + 24.0,
        y + 52.0,
        "Paste the patch link you got from the Echo VR Discord (files.echovr.de).",
        style::body(14.0),
        style::TEXT_DIM,
    );
    let Some(Overlay::PatchLink { id, url }) = &mut d.overlay else {
        return;
    };
    let bw = 110.0;
    let invalid = !url.trim().is_empty() && oauth::validate_dll_url(url.trim()).is_none();
    k.input_with(
        "patch-url",
        url,
        x + 24.0,
        y + 92.0,
        w - 48.0 - bw - 10.0,
        style::MID,
        "https://files.echovr.de/...",
        invalid,
        "The link to your personal pnsovr.dll",
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
            "That doesn't look like a licence patch link.",
            style::body(13.0),
            style::DANGER,
        );
    }
    let valid = oauth::validate_dll_url(url.trim()).is_some();
    let (id, link) = (id.clone(), url.trim().to_string());
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
        "Apply patch",
        x + w - 24.0 - 220.0,
        by,
        220.0,
        style::MID,
        valid && !d.any_job(),
        "Download the patch and put it into this version",
    )
    .clicked
    {
        d.overlay = None;
        patch(d, ctx, &id, Source::Url(link));
    }
}
