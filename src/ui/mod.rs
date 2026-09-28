//! The egui front end: one root window (the main menu) and, on top of it, at most one
//! modal wizard window, each with its own modal dialogs.

mod assets;
mod dialogs;
mod kit;
mod main_menu;
mod parts;
mod pc_install;
mod pc_update;
mod quest_install;
mod quest_update;
mod snapshot;
mod theme;
mod tipbox;
mod wizard;

use main_menu::{MainMenu, Open};
use wizard::{Exit, Wizard, WizardWindow};

pub fn run() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(crate::version::VERSION_TITLE)
            .with_inner_size([main_menu::W, main_menu::H])
            .with_resizable(false)
            .with_maximize_button(false)
            .with_icon(std::sync::Arc::new(assets::icon())),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Echo VR Installer",
        options,
        Box::new(|cc| {
            theme::install_fonts(&cc.egui_ctx);
            theme::install_style(&cc.egui_ctx);
            Ok(Box::new(App {
                snapshots: snapshot::Snapshotter::from_env(),
                ..Default::default()
            }))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

#[derive(Default)]
struct App {
    assets: assets::Assets,
    menu: MainMenu,
    wizard: Option<Box<dyn WizardWindow>>,
    wizard_kind: Option<Open>,
    snapshots: Option<snapshot::Snapshotter>,
    snap_at: Option<(Open, usize, usize)>,
}

impl App {
    fn open(&mut self, which: Open) {
        self.wizard_kind = Some(which);
        self.wizard = Some(match which {
            Open::PcInstall => {
                Box::new(Wizard::new("pc-install", pc_install::PcInstall::default()))
            }
            Open::PcUpdate => Box::new(Wizard::new("pc-update", pc_update::PcUpdate::default())),
            Open::QuestInstall => Box::new(Wizard::new(
                "quest-install",
                quest_install::QuestInstall::default(),
            )),
            Open::QuestUpdate => Box::new(Wizard::new(
                "quest-update",
                quest_update::QuestUpdate::default(),
            )),
        });
    }
}

impl App {
    fn drive_snapshots(&mut self, ctx: &egui::Context) {
        let Some(snap) = self.snapshots.as_mut() else {
            return;
        };
        let want = snap.current().and_then(|s| s.wizard);
        if want != self.snap_at {
            match want {
                Some((open, step, sub)) => {
                    if self.wizard_kind != Some(open) || self.wizard.is_none() {
                        self.open(open);
                    }
                    if let Some(w) = self.wizard.as_mut() {
                        w.debug_goto(step, sub);
                    }
                }
                None => {
                    self.wizard = None;
                    self.wizard_kind = None;
                }
            }
            self.snap_at = want;
            // Wizards are drawn into the root window for snapshots (see `ui`).
            let size = match &self.wizard {
                Some(w) if want.is_some() => egui::vec2(w.width(), wizard::FH),
                _ => egui::vec2(main_menu::W, main_menu::H),
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        }
        let snap = self.snapshots.as_mut().expect("checked");
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
        if self.snapshots.is_some() && self.snap_at.is_some() {
            if let Some(w) = self.wizard.as_mut() {
                w.frame(ui, &self.assets);
                return;
            }
        }
        let own_rect = ui.input(|i| i.viewport().outer_rect);
        let blocked = self.wizard.is_some() || self.menu.dialogs.is_open();
        let mut kit = kit::Kit::new(ui, &self.assets, "main", blocked);
        let open = self.menu.show(&mut kit);
        self.menu.dialogs.show(&ctx, &self.assets, own_rect);
        if let Some(which) = open {
            self.open(which);
        }
        if let Some(w) = self.wizard.as_mut() {
            match w.show(&ctx, &self.assets, own_rect) {
                Some(Exit::Closed) => {
                    self.wizard = None;
                    self.wizard_kind = None;
                }
                Some(Exit::OpenQuestInstall) => self.open(Open::QuestInstall),
                None => {}
            }
        }
    }

    fn on_exit(&mut self) {
        crate::core::elevation::shutdown();
        cleanup_staged_patches();
    }
}

/// Personalized patch files never outlive the session (the Java shutdown hook).
fn cleanup_staged_patches() {
    let dir = crate::core::paths::downloads_dir();
    let _ = std::fs::remove_file(dir.join("pnsovr.dll"));
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("echo_quest_") && name.ends_with(".apk") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}
