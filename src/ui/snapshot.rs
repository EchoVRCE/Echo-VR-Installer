//! Development aid: `ECHOVR_SNAPSHOTS=<dir>` walks the main menu and every wizard step,
//! saves a PNG of each window into `<dir>`, and exits. Used to compare the port against
//! the Java UI without screen-recording permissions.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use egui::{ColorImage, ViewportId};

use super::launcher::{Open, Page, SnapVariant};

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
    pub variant: Option<SnapVariant>,
    pub wizard: Option<(Open, usize, usize)>,
}

pub struct Snapshotter {
    /// `ECHOVR_SNAPSHOTS_DEMO=1`: render the dashboard with made-up versions.
    pub demo: bool,
    dir: PathBuf,
    shots: Vec<Shot>,
    idx: usize,
    frames: u32,
    requested: bool,
    shown_at: Option<std::time::Instant>,
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
            variant: None,
            wizard: None,
        })
        .collect();
        for (n, v) in [
            ("play_menu", SnapVariant::PlayMenu),
            ("play_not_installed", SnapVariant::NotInstalled),
            ("play_installing", SnapVariant::Installing),
            ("play_needs_patch", SnapVariant::NeedsPatch),
            ("setup", SnapVariant::Setup),
        ] {
            shots.push(Shot {
                name: format!("launcher_{n}"),
                page: Page::Play,
                variant: Some(v),
                wizard: None,
            });
        }
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
                    variant: None,
                    wizard: Some((open, s, sub)),
                });
            }
        }
        // `ECHOVR_SNAPSHOTS_ONLY=play,setup`: only shots whose name contains one of these.
        if let Ok(only) = std::env::var("ECHOVR_SNAPSHOTS_ONLY") {
            let terms: Vec<&str> = only.split(',').map(str::trim).collect();
            shots.retain(|s| terms.iter().any(|t| s.name.contains(t)));
        }
        Some(Snapshotter {
            demo: std::env::var_os("ECHOVR_SNAPSHOTS_DEMO").is_some(),
            dir,
            shots,
            idx: 0,
            frames: 0,
            requested: false,
            shown_at: None,
        })
    }

    fn next(&mut self) {
        self.idx += 1;
        self.frames = 0;
        self.requested = false;
        self.shown_at = None;
        *LAST.lock().unwrap() = None;
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
        ctx.request_repaint_after(std::time::Duration::from_millis(30));
        let since = *self.shown_at.get_or_insert_with(std::time::Instant::now);
        let age = since.elapsed().as_millis();
        if age > 10_000 {
            tracing::warn!("snapshot {} never arrived; skipping", shot.name);
            self.next();
            return self.idx >= self.shots.len();
        }
        // Let the page settle (fonts, textures, fades), then ask; re-ask every 1.5 s in
        // case a reply was lost to a window resize.
        let slot = (age.saturating_sub(500) / 1500) as u32;
        if age >= 500 && self.frames != slot + 1 {
            self.frames = slot + 1;
            self.requested = true;
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
                self.next();
            }
        }
        self.idx >= self.shots.len()
    }
}
