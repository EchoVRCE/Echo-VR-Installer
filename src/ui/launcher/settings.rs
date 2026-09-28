//! Settings page: library folder, maintenance, and About (with the Clippy easter egg:
//! double-click the logo).

use super::{Dashboard, Msg, CREDITS, CX, IW, IX};
use crate::core::{paths, platform};
use crate::ui::frame::{self, CONTENT_W, CONTENT_X};
use crate::ui::kit::{Btn, Kit};
use crate::ui::parts;
use crate::ui::style;
use crate::ui::theme;

fn title(kit: &Kit, text: &str, y: f32) {
    kit.header(text, CX - 150.0, y, 300.0, 34.0);
}

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    // Library.
    title(kit, "Library", 64.0);
    let (fw, bw) = (560.0, Btn::Small.w());
    let x0 = CX - (fw + 10.0 + bw) / 2.0;
    if kit
        .text_field(
            "library",
            &mut d.library_field,
            x0,
            106.0,
            fw,
            25.0,
            12.0,
            "",
            theme::FIELD_BG,
            "Where new versions are installed",
        )
        .committed
    {
        set_library(d);
    }
    if kit.button(
        "lib-browse",
        Btn::Small,
        "Browse",
        11.0,
        x0 + fw + 10.0,
        106.0,
        !d.any_job(),
        "Pick the library folder",
    ) {
        if let Some(p) = parts::choose_folder() {
            d.library_field = p;
            set_library(d);
        }
    }
    kit.text_fit_center(
        IX,
        136.0,
        IW,
        16.0,
        "New versions get their own folder here. Existing installs stay where they are.",
        theme::arial(12.0),
        style::TEXT_DIM,
    );

    // Maintenance.
    title(kit, "Maintenance", 166.0);
    let (mw, gap) = (Btn::Middle.w(), 12.0);
    let x0 = CX - (3.0 * mw + 2.0 * gap) / 2.0;
    if kit.button(
        "del-cache",
        Btn::Middle,
        "Delete cache",
        14.0,
        x0,
        208.0,
        !d.deleting_cache,
        "Clear cached downloads to free up space",
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
        14.0,
        x0 + mw + gap,
        208.0,
        true,
        "The launcher's log files",
    ) {
        open_dir(d, &paths::log_dir());
    }
    if kit.button(
        "data",
        Btn::Middle,
        "Open data folder",
        14.0,
        x0 + 2.0 * (mw + gap),
        208.0,
        true,
        "launcher.json and logs",
    ) {
        open_dir(d, &paths::data_dir());
    }

    // About.
    title(kit, "About", 262.0);
    let name = "Echo VR Launcher";
    let nw = kit.text_width(name, theme::conthrax(16.0));
    let lx = (CX - (48.0 + 14.0 + nw) / 2.0).floor();
    kit.image("icon.png", lx, 304.0, 48.0, 48.0);
    if kit
        .hand_area("about-logo", lx, 304.0, 48.0, 48.0, "")
        .hovered
        && kit.ui.input(|i| {
            i.pointer
                .button_double_clicked(egui::PointerButton::Primary)
        })
    {
        d.clippy.trigger(ctx);
    }
    kit.text(lx + 62.0, 306.0, name, theme::conthrax(16.0), style::TEXT);
    kit.text(
        lx + 62.0,
        332.0,
        crate::version::VERSION_TITLE,
        theme::arial(13.0),
        style::TEXT_DIM,
    );
    kit.text_center(
        IX,
        362.0,
        IW,
        128.0,
        CREDITS,
        theme::arial(12.0),
        style::TEXT,
        None,
    );

    kit.text_center(
        CONTENT_X,
        frame::BAR_Y,
        CONTENT_W,
        frame::BAR_H,
        crate::version::VERSION_TITLE,
        theme::conthrax(11.0),
        theme::LIGHT_GRAY,
        None,
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
