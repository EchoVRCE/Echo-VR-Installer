//! Development aid: `ECHOVR_SNAPSHOTS=<dir>` walks the main menu and every wizard step,
//! saves a PNG of each window into `<dir>`, and exits. Used to compare the port against
//! the Java UI without screen-recording permissions.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use egui::{ColorImage, ViewportId};

use super::launcher::{Open, Page};

static LAST: Mutex<Option<(ViewportId, Arc<ColorImage>)>> = Mutex::new(None);

/// Called by every viewport each frame: keeps a screenshot reply if one arrived.
pub fn capture(ui: &egui::Ui) {
    ui.input(|i| {
        for e in &i.raw.events {
            if let egui::Event::Screenshot {
                viewport_id, image, ..
            } = e
            {
                *LAST.lock().unwrap() = Some((*viewport_id, image.clone()));
            }
        }
    });
}

#[derive(Debug, Clone)]
pub struct Shot {
    pub name: String,
    pub page: Page,
    pub wizard: Option<(Open, usize, usize)>,
}

pub struct Snapshotter {
    dir: PathBuf,
    shots: Vec<Shot>,
    idx: usize,
    frames: u32,
    requested: bool,
}

impl Snapshotter {
    pub fn from_env() -> Option<Snapshotter> {
        let dir = PathBuf::from(std::env::var_os("ECHOVR_SNAPSHOTS")?);
        std::fs::create_dir_all(&dir).ok()?;
        let mut shots: Vec<Shot> = [
            ("play", Page::Play),
            ("versions", Page::Versions),
            ("mods", Page::Mods),
            ("servers", Page::Servers),
            ("settings", Page::Settings),
        ]
        .into_iter()
        .map(|(n, page)| Shot {
            name: format!("launcher_{n}"),
            page,
            wizard: None,
        })
        .collect();
        let wiz = [
            (
                Open::PcInstall,
                "pc_install",
                vec![(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)],
            ),
            (Open::PcUpdate, "pc_update", vec![(0, 0), (1, 0), (2, 0)]),
            (
                Open::QuestInstall,
                "quest_install",
                vec![(0, 0), (1, 0), (2, 0), (3, 0)],
            ),
            (
                Open::QuestUpdate,
                "quest_update",
                vec![(0, 0), (1, 0), (2, 0)],
            ),
        ];
        for (open, name, steps) in wiz {
            for (s, sub) in steps {
                shots.push(Shot {
                    name: format!("{name}_{s}_{sub}"),
                    page: Page::Play,
                    wizard: Some((open, s, sub)),
                });
            }
        }
        Some(Snapshotter {
            dir,
            shots,
            idx: 0,
            frames: 0,
            requested: false,
        })
    }

    pub fn current(&self) -> Option<&Shot> {
        self.shots.get(self.idx)
    }

    /// Drives one frame. `target` is the viewport the current shot belongs to.
    /// Returns true when all shots are done.
    pub fn tick(&mut self, ctx: &egui::Context, target: ViewportId) -> bool {
        let Some(shot) = self.shots.get(self.idx).cloned() else {
            return true;
        };
        ctx.request_repaint();
        self.frames += 1;
        // Re-request if a reply got lost (e.g. the window was resized meanwhile).
        if self.frames % 60 == 20 {
            self.requested = true;
            *LAST.lock().unwrap() = None;
            ctx.send_viewport_cmd_to(
                target,
                egui::ViewportCommand::Screenshot(Default::default()),
            );
        }
        let got = LAST.lock().unwrap().take();
        if let Some((vid, img)) = got {
            if vid == target {
                let path = self.dir.join(format!("{}.png", shot.name));
                let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Some(buf) =
                    image::RgbaImage::from_raw(img.size[0] as u32, img.size[1] as u32, rgba)
                {
                    let _ = buf.save(&path);
                }
                tracing::info!("snapshot {}", path.display());
                self.idx += 1;
                self.frames = 0;
                self.requested = false;
            }
        }
        self.idx >= self.shots.len()
    }
}
