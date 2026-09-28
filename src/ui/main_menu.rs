//! `FrameMain`: the start window.

use egui::{vec2, Color32, Stroke};

use super::dialogs::DialogHost;
use super::kit::{Btn, Kit};
use super::parts::Worker;
use super::theme;
use super::tipbox::{self, TipBox};

pub const W: f32 = 1280.0;
pub const H: f32 = 720.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Open {
    PcInstall,
    PcUpdate,
    QuestInstall,
    QuestUpdate,
}

enum Msg {
    CacheDeleted(Vec<std::path::PathBuf>),
}

#[derive(Default)]
pub struct MainMenu {
    tipbox: TipBox,
    pub dialogs: DialogHost,
    worker: Worker<Msg>,
    deleting: bool,
}

const CREDITS: &str = "Copyright for Echo VR is by Meta/Ready at Dawn!\n\
This installer is not at all associated with them!\n\n\
Special thanks to Sick and SirDominik for some of the backgrounds!\n\
Special thanks to F-A-N-G-O-R-N for getting me into Java and helping with this project.\n\
I know you still feel shame when you have to look at my source code.\n\
Special thanks to Leon(leon1273) for contributing and cleaning stuff in my code\n\
This tool is still in early alpha!\n\
If you have problems, contact me on Discord 'marshmallow_mia'.";

impl MainMenu {
    /// Draws the menu; returns a wizard to open.
    pub fn show(&mut self, kit: &mut Kit) -> Option<Open> {
        for Msg::CacheDeleted(failed) in self.worker.drain() {
            self.deleting = false;
            let mut msg = String::from("The cached files have been deleted.");
            if !failed.is_empty() {
                msg.push_str("\n\nThese could not be deleted (in use or no permission):");
                for f in failed.iter().take(6) {
                    msg.push_str(&format!("\n{}", f.display()));
                }
            }
            self.dialogs.info("Deleting done", &msg);
        }

        let tip_x = ((W - tipbox::W) / 2.0).floor();
        let tip_y = 240.0;
        kit.image("Echox720.png", 0.0, 0.0, W, H);
        self.tipbox.draw_clippy(kit, tip_x, tip_y);

        let mut open = None;
        let left = ((W / 2.0 - Btn::Big.w()) / 2.0).floor(); // 179
        if kit.button(
            "pc-install",
            Btn::Big,
            "Install Echo VR",
            20.0,
            left,
            200.0,
            true,
            "Download and install Echo VR for PC",
        ) {
            open = Some(Open::PcInstall);
        }
        if kit.button(
            "pc-update",
            Btn::Middle,
            "Update Echo (PC)",
            15.0,
            left,
            280.0,
            true,
            "Download and install the latest Echo VR game update",
        ) {
            open = Some(Open::PcUpdate);
        }
        if kit.button(
            "quest-install",
            Btn::Big,
            "Quest Install Echo",
            20.0,
            819.0,
            200.0,
            true,
            "Download and install Echo VR on your Quest headset",
        ) {
            open = Some(Open::QuestInstall);
        }
        if kit.button(
            "quest-update",
            Btn::Middle,
            "Update Echo (Quest)",
            15.0,
            819.0,
            280.0,
            true,
            "Download and install the latest Echo VR update on your Quest",
        ) {
            open = Some(Open::QuestUpdate);
        }
        if kit.button(
            "delete-cache",
            Btn::Middle,
            "Delete cache",
            17.0,
            818.0,
            595.0,
            !self.deleting,
            "Clear cached downloaded files to free up space",
        ) {
            self.deleting = true;
            let ctx = kit.ctx();
            self.worker.spawn(&ctx, |tx| {
                tx.send(Msg::CacheDeleted(crate::core::cache::delete_all()))
            });
        }

        // Credits badge: a vector "ⓘ".
        let (bx, by, s) = (W - 40.0 - 30.0, 595.0, 40.0);
        let c = kit.rect(bx, by, s, s).center();
        let p = kit.ui.painter();
        p.circle_filled(c, s / 2.0 - 1.0, Color32::from_rgb(200, 0, 150));
        p.circle_stroke(
            c,
            s / 2.0 - 1.5,
            Stroke::new((s / 22.0).max(1.5), theme::rgba(255, 255, 255, 190)),
        );
        let dot_r = (s / 12.0).floor().max(2.0);
        p.circle_filled(
            c + vec2(0.0, (s * 0.27).round() - s / 2.0),
            dot_r,
            Color32::WHITE,
        );
        p.line_segment(
            [
                c + vec2(0.0, (s * 0.43).round() - s / 2.0),
                c + vec2(0.0, (s * 0.73).round() - s / 2.0),
            ],
            Stroke::new((s / 9.0).max(2.0), Color32::WHITE),
        );
        if kit
            .hand_area("credits", bx, by, s, s, "About this installer & credits")
            .clicked
        {
            self.dialogs.info("Credits", CREDITS);
        }

        // Invisible easter egg.
        if kit
            .area("easter-egg", 590.0, 430.0, 100.0, 100.0, "")
            .clicked
        {
            self.dialogs
                .info("You found an Easter Egg", "Never divide by 0!");
        }

        self.tipbox.draw(kit, tip_x, tip_y);
        open
    }
}
