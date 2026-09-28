//! Settings page: library folder, maintenance, and About (with the Clippy easter egg).

use super::{Dashboard, Msg, CREDITS, X0};
use crate::core::{paths, platform};
use crate::ui::kit::Kit;
use crate::ui::parts;
use crate::ui::style::{self, Icon, Variant};

const W: f32 = 760.0;

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    let x = X0;

    // Library.
    let y = 112.0;
    kit.titled_card(x, y, W, 128.0, "Library");
    let tip = format!("Where new versions are installed:\n{}", d.library_field);
    if kit.input_with(
        "library",
        &mut d.library_field,
        x + 20.0,
        y + 47.0,
        W - 172.0,
        style::MID,
        "",
        false,
        &tip,
        style::body(14.0),
    ) {
        set_library(d);
    }
    if kit
        .flat_button(
            "lib-browse",
            Variant::Secondary,
            Some(Icon::Folder),
            "Browse",
            x + W - 140.0,
            y + 47.0,
            120.0,
            style::MID,
            !d.any_job(),
            "Pick the library folder",
        )
        .clicked
    {
        if let Some(p) = parts::choose_folder() {
            d.library_field = p;
            set_library(d);
        }
    }
    kit.text(
        x + 20.0,
        y + 98.0,
        "New versions get their own folder here. Existing installs stay where they are.",
        style::body(12.0),
        style::TEXT_MUTED,
    );

    // Maintenance.
    let y = 256.0;
    kit.titled_card(x, y, W, 112.0, "Maintenance");
    let bw = (W - 40.0 - 24.0) / 3.0;
    if kit
        .flat_button(
            "del-cache",
            Variant::Secondary,
            Some(Icon::Refresh),
            "Delete cache",
            x + 20.0,
            y + 50.0,
            bw,
            40.0,
            !d.deleting_cache,
            "Clear cached downloads to free up space",
        )
        .clicked
    {
        d.deleting_cache = true;
        d.worker.spawn(ctx, |tx| {
            tx.send(Msg::CacheDeleted(crate::core::cache::delete_all()))
        });
    }
    if kit
        .flat_button(
            "logs",
            Variant::Secondary,
            Some(Icon::Folder),
            "Open logs",
            x + 32.0 + bw,
            y + 50.0,
            bw,
            40.0,
            true,
            "The launcher's log files",
        )
        .clicked
    {
        open_dir(d, &paths::log_dir());
    }
    if kit
        .flat_button(
            "data",
            Variant::Secondary,
            Some(Icon::Folder),
            "Open data folder",
            x + 44.0 + 2.0 * bw,
            y + 50.0,
            bw,
            40.0,
            true,
            "launcher.json and logs",
        )
        .clicked
    {
        open_dir(d, &paths::data_dir());
    }

    // Play setup: what the first-run setup asked, and launching.
    let y = 384.0;
    kit.titled_card(x, y, W, 132.0, "Play setup");
    kit.caps(x + 20.0, y + 54.0, "Echo VR licence", style::TEXT_MUTED);
    let owner = match d.state.owner {
        Some(true) => 0,
        Some(false) => 1,
        None => usize::MAX,
    };
    if let Some(i) = kit.segmented(
        "settings-owner",
        &["I own it", "New player"],
        owner,
        x + 200.0,
        y + 46.0,
        300.0,
        30.0,
    ) {
        d.state.owner = Some(i == 0);
        d.state.setup_done = true;
        d.save();
    }
    kit.text(
        x + 516.0,
        y + 53.0,
        match d.state.owner {
            Some(false) => "PLAY asks for the licence patch first.",
            Some(true) => "Your own licence; the patch is optional.",
            None => "Not set yet.",
        },
        style::body(12.0),
        style::TEXT_MUTED,
    );
    if kit.toggle(
        "minimize",
        &mut d.state.minimize_on_launch,
        "Minimize the launcher when Echo VR starts",
        x + 20.0,
        y + 90.0,
        true,
        "Keeps the launcher out of the way while you play",
    ) {
        d.save();
    }
    if kit.toggle(
        "artwork",
        &mut d.state.revive_artwork,
        "SteamVR: add game artwork",
        x + 440.0,
        y + 90.0,
        true,
        "When setting up SteamVR, also install Echo VR's artwork for the SteamVR library",
    ) {
        d.save();
    }

    // About -- Clippy rises from behind the card's top edge.
    let y = 532.0;
    d.clippy.draw(kit, x + W - 200.0, y, 180.0);
    kit.titled_card(x, y, W, 116.0, "About");
    kit.image("icon.png", x + 20.0, y + 48.0, 48.0, 48.0);
    let logo = kit.rect(x + 20.0, y + 48.0, 48.0, 48.0);
    if !kit.blocked {
        let r = kit
            .ui
            .interact(logo, egui::Id::new("about-logo"), egui::Sense::click());
        if r.double_clicked() {
            d.clippy.trigger(ctx);
        }
    }
    kit.text(
        x + 84.0,
        y + 50.0,
        "Echo VR Launcher",
        style::display(18.0),
        style::TEXT,
    );
    kit.text(
        x + 84.0,
        y + 78.0,
        crate::version::VERSION_TITLE,
        style::body(13.0),
        style::TEXT_DIM,
    );
    if kit
        .flat_button(
            "credits",
            Variant::Secondary,
            None,
            "Credits",
            x + W - 150.0,
            y + 60.0,
            130.0,
            style::SMALL,
            true,
            "Who made this possible",
        )
        .clicked
    {
        d.dialogs.info("Credits", CREDITS);
    }
}

fn open_dir(d: &mut Dashboard, dir: &std::path::Path) {
    let _ = std::fs::create_dir_all(dir);
    if let Err(e) = platform::open_folder(dir) {
        d.dialogs.error(
            "Couldn't open folder",
            &format!("{e:#}"),
            Default::default(),
        );
    }
}

fn set_library(d: &mut Dashboard) {
    let lib = paths::normalize(&d.library_field);
    if lib.is_empty() {
        d.library_field = d.state.library.clone();
        return;
    }
    d.library_field = lib.clone();
    if lib != d.state.library {
        d.state.library = lib;
        d.save();
    }
}
