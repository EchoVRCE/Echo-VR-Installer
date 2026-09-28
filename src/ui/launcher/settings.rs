//! Settings page: library folder, cache, logs.

use super::{header, label, Dashboard, Msg, PAGE_W};
use crate::core::{paths, platform};
use crate::ui::kit::{Btn, Kit};
use crate::ui::launcher::Open;
use crate::ui::parts;
use crate::ui::theme;

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    header(kit, "Settings");
    let x = 110.0;
    label(kit, x, 70.0, "Library folder");
    let r = kit.text_field(
        "library",
        &mut d.library_field,
        x,
        94.0,
        640.0,
        24.0,
        12.0,
        "",
        theme::FIELD_BG,
        "Where new versions are installed",
    );
    if r.committed {
        set_library(d);
    }
    if kit.button(
        "lib-choose",
        Btn::Small,
        "Choose path",
        11.0,
        x + 650.0,
        93.0,
        !d.any_job(),
        "Pick the library folder",
    ) {
        if let Some(p) = parts::choose_folder() {
            d.library_field = p;
            set_library(d);
        }
    }
    kit.text_left(
        x,
        124.0,
        20.0,
        "New versions are installed into their own folder here. Existing installs stay where they are.",
        theme::arial(12.0),
        theme::LIGHT_GRAY,
    );

    label(kit, x, 170.0, "Maintenance");
    if kit.button(
        "del-cache",
        Btn::Middle,
        "Delete cache",
        16.0,
        x,
        196.0,
        !d.deleting_cache,
        "Clear cached downloaded files to free up space",
    ) {
        d.deleting_cache = true;
        d.worker.spawn(ctx, |tx| {
            tx.send(Msg::CacheDeleted(crate::core::cache::delete_all()))
        });
    }
    if kit.button(
        "logs",
        Btn::Middle,
        "Open logs",
        16.0,
        x + 230.0,
        196.0,
        true,
        "Open the folder with the launcher's log files",
    ) {
        let _ = std::fs::create_dir_all(paths::log_dir());
        if let Err(e) = platform::open_folder(&paths::log_dir()) {
            d.dialogs.error(
                "Couldn't open folder",
                &format!("{e:#}"),
                Default::default(),
            );
        }
    }
    if kit.button(
        "data",
        Btn::Middle,
        "Open data folder",
        14.0,
        x + 460.0,
        196.0,
        true,
        "Launcher settings (launcher.json) and logs",
    ) {
        let _ = std::fs::create_dir_all(paths::data_dir());
        if let Err(e) = platform::open_folder(&paths::data_dir()) {
            d.dialogs.error(
                "Couldn't open folder",
                &format!("{e:#}"),
                Default::default(),
            );
        }
    }
    kit.text_center(
        0.0,
        300.0,
        PAGE_W,
        60.0,
        crate::version::VERSION_TITLE,
        theme::conthrax(13.0),
        theme::LIGHT_GRAY,
        None,
    );
    None
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
