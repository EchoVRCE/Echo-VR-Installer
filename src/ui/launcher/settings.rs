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
    if kit.input(
        "library",
        &mut d.library_field,
        x + 20.0,
        y + 46.0,
        W - 172.0,
        40.0,
        "",
        false,
        "Where new versions are installed",
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
            y + 46.0,
            120.0,
            40.0,
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

    // About -- Clippy rises from behind the card's top edge.
    let y = 384.0;
    d.clippy.draw(kit, x + W - 200.0, y, 180.0);
    kit.titled_card(x, y, W, 296.0, "About");
    kit.image("icon.png", x + 20.0, y + 46.0, 48.0, 48.0);
    let logo = kit.rect(x + 20.0, y + 46.0, 48.0, 48.0);
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
        y + 48.0,
        "Echo VR Launcher",
        style::display(18.0),
        style::TEXT,
    );
    kit.text(
        x + 84.0,
        y + 76.0,
        crate::version::VERSION_TITLE,
        style::body(13.0),
        style::TEXT_DIM,
    );
    let mut job = egui::text::LayoutJob::simple(
        CREDITS.to_string(),
        style::body(13.0),
        style::TEXT_DIM,
        W - 40.0,
    );
    job.halign = egui::Align::LEFT;
    let g = kit.ui.ctx().fonts_mut(|f| f.layout_job(job));
    kit.ui.painter().galley(
        kit.rect(x + 20.0, y + 114.0, 0.0, 0.0).min,
        g,
        style::TEXT_DIM,
    );
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
