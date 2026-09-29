//! The egui front end: the launcher in one window, plus modal dialog windows. The
//! installer's steps (install, patch, SteamVR setup, Quest) run inside the launcher.

mod assets;
mod design;
mod dialogs;
#[cfg(test)]
mod headless;
mod kit;
mod launcher;
mod markdown;
mod parts;
mod snapshot;
mod style;
mod theme;
mod tipbox;

use launcher::Dashboard;

pub fn run() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(crate::version::VERSION_TITLE)
            .with_inner_size([launcher::W, launcher::H])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_icon(std::sync::Arc::new(assets::icon())),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Echo VR Launcher",
        options,
        Box::new(|cc| {
            theme::install_fonts(&cc.egui_ctx);
            theme::install_style(&cc.egui_ctx);
            let snapshots = snapshot::Snapshotter::from_env();
            let mut app = App::default();
            app.menu.demo = snapshots.as_ref().is_some_and(|s| s.demo);
            app.snapshots = snapshots;
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

#[derive(Default)]
struct App {
    assets: assets::Assets,
    menu: Dashboard,
    snapshots: Option<snapshot::Snapshotter>,
}

impl App {
    fn drive_snapshots(&mut self, ctx: &egui::Context) {
        let Some(snap) = self.snapshots.as_mut() else {
            return;
        };
        if let Some(shot) = snap.current() {
            self.menu.page = shot.page;
            self.menu.snap_variant = shot.variant;
        }
        if snap.tick(ctx, egui::ViewportId::ROOT) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        snapshot::capture(ui);
        self.drive_snapshots(&ctx);
        let own_rect = ui.input(|i| i.viewport().outer_rect);
        let blocked = self.menu.dialogs.is_open();
        let mut kit = kit::Kit::new(ui, &self.assets, "main", blocked);
        self.menu.show(&mut kit);
        self.menu.dialogs.show(&ctx, &self.assets, own_rect);
    }

    fn on_exit(&mut self) {
        crate::core::elevation::shutdown();
        cleanup_staged_patches();
    }
}

/// Personal patch files never outlive the session (the Java shutdown hook).
fn cleanup_staged_patches() {
    let dir = crate::core::paths::downloads_dir();
    let _ = std::fs::remove_file(dir.join("pnsovr.dll"));
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            // Personal patched APKs go; the stock APK stays for the next install.
            if name.ends_with(".patched.apk") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}
