//! Play page, as the design concept has it: the ECHO VR logo over an info line, PLAY,
//! CHECK FOR UPDATES and the PCVR|QUEST switch, Community News (a banner and two cards)
//! and SERVER INFO on the right. PLAY does what the selected version needs (install,
//! patch, set up SteamVR, play, stop); on the Quest side it connects, installs or starts
//! Echo VR on the headset.

use egui::text::{LayoutJob, TextFormat};
use egui::Color32;

use super::{server_info, setup, versions, Dashboard, Msg};
use crate::core::adb::devices::Status;
use crate::core::error::UiError;
use crate::core::launcher::catalog::{Platform, VersionEntry};
use crate::core::launcher::feed::NewsItem;
use crate::core::launcher::store::{InstalledVersion, Runtime, Target};
use crate::core::launcher::{launch, quest};
use crate::core::{paths, revive};
use crate::ui::design::{self, dz, Dr};
use crate::ui::dialogs::Icon as DlgIcon;
use crate::ui::kit::Kit;
use crate::ui::markdown::{self, Look};
use crate::ui::style::{self, Variant};

const LAUNCH_ANYWAY: &str = "launch-anyway";

// Geometry in design pixels. The image rects put each button's shape where the concept
// has it: play_button.png's shape sits at (61, 61) in its 821×380 glow, update_button.png
// is just its shape, and the hardware switch's body starts 74 px under its labels.
const LOGO: Dr = Dr::new(119.9, 74.4, 747.7, 126.5);
/// Vertical centre of the info line, and the size of paths on it (Myriad).
const INFO_Y: f32 = 217.0;
const PATH_SIZE: f32 = 16.6;
const PLAY_S: f32 = 0.2597;
const PLAY_IMG: Dr = Dr::new(123.2, 227.2, 821.0 * PLAY_S, 380.0 * PLAY_S);
const PLAY_SHAPE: [(f32, f32); 4] = [
    (139.0, 243.0),
    (269.4, 243.0),
    (332.0, 309.8),
    (139.0, 309.8),
];
/// Where PLAY reacts: its box, cut where CHECK FOR UPDATES' slant begins.
const PLAY_AREA: Dr = Dr::new(139.0, 243.0, 186.0, 67.0);
/// The flat part of PLAY a label has to fit in, and the width of the image's "PLAY".
const PLAY_LABEL: Dr = Dr::new(149.0, 243.0, 150.0, 67.0);
const PLAY_WORD_W: f32 = 102.0;
const UPDATE_IMG: Dr = Dr::new(320.1, 243.0, 342.9, 66.0);
const UPDATE_SHAPE: [(f32, f32); 4] = [
    (320.1, 243.0),
    (663.0, 243.0),
    (663.0, 309.0),
    (383.2, 309.0),
];
const UPDATE_AREA: Dr = Dr::new(325.0, 243.0, 338.0, 66.0);
const SWITCH_IMG: Dr = Dr::new(697.0, 248.4, 147.0, 60.1);
const SWITCH_PC: Dr = Dr::new(697.0, 265.0, 73.5, 43.0);
const SWITCH_QUEST: Dr = Dr::new(770.5, 265.0, 73.5, 43.0);
/// A running job's progress, in place of PLAY and CHECK FOR UPDATES.
const JOB_ROW: Dr = Dr::new(139.0, 243.0, 524.0, 67.0);
const NEWS: Dr = Dr::new(138.0, 350.0, 1144.0, 421.0);
/// news_header.png is the top 75 of CommunityNewsTab's 697 px.
const NEWS_HEADER_H: f32 = 421.0 * 75.0 / 697.0;
/// The news link's right end and baseline, at the banner's bottom right.
const NEWS_LINK: (f32, f32) = (1226.0, 727.0);
const CARDS: [Dr; 2] = [
    Dr::new(137.0, 800.0, 555.0, 248.0),
    Dr::new(728.0, 800.0, 555.0, 248.0),
];

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    // Look for a headset quietly once, so the Quest chip has something to say.
    if !d.quest_auto_checked && !d.demo {
        d.quest_auto_checked = true;
        if d.quest_conn.status.is_none() && !d.quest_conn.checking {
            d.check_quest(ctx, false);
        }
    }
    if d.dialogs.take(LAUNCH_ANYWAY).is_some_and(|a| a.is_yes()) {
        start(d, ctx);
    }
    kit.image_d("logo_echovr.png", LOGO);
    let a = match d.play_platform {
        Platform::Pc => pc_action(d),
        Platform::Quest => quest_action(d),
    };
    info_line(kit, &a);
    buttons(d, kit, ctx, a);
    switch(d, kit);
    news(d, kit);
    server_info::show(d, kit);
}

/// What PLAY does.
enum Main {
    Play,
    Stop,
    Install(VersionEntry),
    Reinstall(VersionEntry),
    Patch(String),
    SetUpRevive,
    QuestConnect,
    QuestInstall,
    QuestPlay,
    Nothing,
}

/// What CHECK FOR UPDATES does.
enum Update {
    Pc(InstalledVersion),
    Quest,
    Nothing,
}

/// Everything the top of the page shows for the current side.
struct Action {
    /// The info line: the state first, then details.
    info: Vec<String>,
    info_color: Color32,
    label: &'static str,
    main: Main,
    enabled: bool,
    tip: String,
    /// A job running for this target: its id, progress label and fraction.
    job: Option<(String, String, Option<f32>)>,
    update: Update,
    update_enabled: bool,
    /// The last update failed: the button shows its orange "!".
    update_alert: bool,
}

impl Action {
    fn new() -> Action {
        Action {
            info: Vec::new(),
            info_color: design::GREY,
            label: "PLAY",
            main: Main::Nothing,
            enabled: false,
            tip: String::new(),
            job: None,
            update: Update::Nothing,
            update_enabled: false,
            update_alert: false,
        }
    }

    /// A problem the player has to fix, in red at the end of the info line.
    fn problem(&mut self, text: impl Into<String>) {
        self.info.push(text.into());
        self.info_color = design::DANGER;
    }
}

fn gb(bytes: u64) -> String {
    let g = bytes as f64 / 1e9;
    if g >= 10.0 || (g - g.round()).abs() < 0.05 {
        format!("{g:.0}GB")
    } else {
        format!("{g:.1}GB")
    }
}

fn versions_root(d: &Dashboard, id: &str) -> String {
    crate::core::launcher::versions::root_for(&d.state.library, id)
}

// ---- PC ----

fn pc_action(d: &mut Dashboard) -> Action {
    let target = d.target();
    let id = match &target {
        Target::Installed(v) | Target::Missing(v) => Some(v.id.clone()),
        Target::Available(e) => Some(e.id.clone()),
        Target::None => None,
    };
    let job_id = id.filter(|id| d.jobs.contains_key(id)).or_else(|| {
        d.jobs
            .contains_key(setup::REVIVE_JOB)
            .then(|| setup::REVIVE_JOB.into())
    });
    let mut a = Action::new();
    a.job = job_id.and_then(|id| {
        let j = d.jobs.get(&id)?;
        Some((id, j.label.clone(), j.fraction))
    });
    let needs_revive = d.state.profile.runtime == Runtime::Revive
        && (cfg!(windows) || d.demo)
        && d.revive_dir().is_none();
    let running = d.game().is_running();
    let ours = d.child.is_some();
    let size_of = |d: &Dashboard, cid: Option<&String>| {
        let c = d.catalog.as_ref()?;
        c.pc().find(|e| Some(&e.id) == cid)?.size.map(gb)
    };
    match target {
        Target::Installed(v) => {
            let size = size_of(d, v.catalog_id.as_ref());
            let state = if running {
                "Running"
            } else if d.state.owner == Some(false) && !v.patched {
                "Needs the licence patch"
            } else if needs_revive {
                "SteamVR is not set up"
            } else {
                "Installed"
            };
            a.info.push(state.into());
            a.info.push(v.name.clone());
            a.info.extend(size);
            a.info.push(v.root.clone());
            if running && ours {
                (a.label, a.main, a.enabled) = ("STOP", Main::Stop, true);
                a.tip = "Close Echo VR".into();
            } else if running {
                a.label = "RUNNING";
                a.tip = "Echo VR was started outside the launcher.".into();
            } else if d.state.owner == Some(false) && !v.patched {
                (a.label, a.main) = ("PATCH", Main::Patch(v.id.clone()));
                a.enabled = !d.any_job();
                a.tip = "New players need a personal licence patch: authorize with Discord to get yours.".into();
            } else if needs_revive {
                (a.label, a.main) = ("SET UP STEAMVR", Main::SetUpRevive);
                a.enabled = !d.any_job();
                a.tip = "Installs Revive, which runs Echo VR on SteamVR (asks for administrator rights)".into();
            } else {
                (a.main, a.enabled) = (Main::Play, true);
                a.tip = "Start Echo VR".into();
            }
            a.update_enabled = !running && !d.any_job();
            a.update_alert = d
                .update_note
                .get(&v.id)
                .is_some_and(|n| n.contains("failed"));
            a.update = Update::Pc(v);
        }
        Target::Missing(v) => {
            a.info.push("Game files missing".into());
            a.info.push(v.name.clone());
            a.info_color = design::DANGER;
            let entry = v.catalog_id.as_ref().and_then(|cid| {
                d.catalog
                    .as_ref()
                    .and_then(|c| c.pc().find(|e| &e.id == cid).cloned())
            });
            match entry {
                Some(e) if !v.external => {
                    a.info.push(v.root.clone());
                    a.label = "REINSTALL";
                    a.tip = format!(
                        "Download this version again into its folder ({})",
                        e.size.map(gb).unwrap_or_default()
                    );
                    a.main = Main::Reinstall(e);
                    a.enabled = !d.any_job();
                }
                _ => a.problem(format!(
                    "{} is gone: add its new location on the Versions page",
                    v.root
                )),
            }
        }
        Target::Available(e) => {
            let installing = a.job.is_some();
            a.info.push(
                if installing {
                    "Installing"
                } else {
                    "Not installed"
                }
                .into(),
            );
            a.info.push(e.name.clone());
            a.info.extend(e.size.map(gb));
            a.info.push(versions_root(d, &e.id));
            let free = d.free_bytes();
            let short = matches!((e.size, free), (Some(need), Some(free)) if need > free);
            if let (true, Some(free)) = (short, free) {
                a.problem(format!(
                    "Not enough space: {} free. Pick another library in Settings",
                    gb(free)
                ));
            }
            a.label = "INSTALL";
            a.tip = "Download and install this version into your library".into();
            a.enabled = !short && !d.any_job();
            a.main = Main::Install(e);
        }
        Target::None => {
            a.info.push("Loading the version list".into());
        }
    }
    a
}

// ---- Quest ----

fn quest_action(d: &mut Dashboard) -> Action {
    let ready = d.quest_conn.status == Some(Status::Ready);
    let installed = ready && d.quest_info.as_ref().is_some_and(|i| i.installed);
    let known = ready && d.quest_info.is_some();
    let busy = d.quest_conn.checking || d.quest_busy || d.any_job();
    let mut a = Action::new();
    a.job = d
        .jobs
        .get(setup::QUEST_JOB)
        .map(|j| (setup::QUEST_JOB.to_string(), j.label.clone(), j.fraction));
    let device = d.quest_info.as_ref().and_then(|i| i.device.clone());
    a.info = match (d.quest_conn.checking, d.quest_conn.status) {
        (true, _) => vec!["Looking for your headset over USB".into()],
        (_, Some(Status::Ready)) => {
            let mut parts = match &d.quest_info {
                Some(i) if i.installed => vec!["Installed".into(), i.version_label()],
                Some(_) => vec!["Not installed on this Quest".into()],
                None => vec!["Reading the installed version".into()],
            };
            parts.extend(device);
            parts
        }
        (_, Some(Status::Unauthorized)) => vec![
            "Allow this PC".into(),
            "Accept the USB debugging prompt in the headset".into(),
        ],
        (_, Some(Status::Ambiguous)) => vec![
            "Several devices connected".into(),
            "Pick your Quest when you connect".into(),
        ],
        (_, Some(Status::None)) => vec![
            "No Quest found".into(),
            "Plug it in by USB with developer mode on".into(),
        ],
        (_, None) => vec!["Plug in your Quest by USB".into()],
    };
    if installed {
        (a.main, a.enabled) = (Main::QuestPlay, !d.quest_busy);
        a.tip = "Start Echo VR on the headset".into();
    } else if known {
        (a.label, a.main, a.enabled) = ("INSTALL", Main::QuestInstall, !busy);
        a.tip = "Download Echo VR and install it on your Quest".into();
    } else {
        a.label = "CONNECT";
        a.main = Main::QuestConnect;
        a.enabled = !d.quest_conn.checking && !d.quest_busy;
        a.tip = "Look for your Quest over USB".into();
    }
    a.update = Update::Quest;
    a.update_enabled = installed && !busy;
    a
}

// ---- drawing ----

/// Part of the info line in DMCAPS, or in Myriad for folder paths: DMCAPS has no
/// lowercase, and the design sets paths as they are. Myriad's "-" and "_" vanish at small
/// sizes (around 11 px), so those two come from Liberation Sans.
fn info_part(job: &mut LayoutJob, text: &str, color: Color32) {
    let format = |font| TextFormat {
        font_id: font,
        color,
        extra_letter_spacing: dz(0.5),
        valign: egui::Align::Center,
        ..Default::default()
    };
    if !(text.contains('/') || text.contains('\\')) {
        job.append(&text.to_uppercase(), 0.0, format(design::din(15.2)));
        return;
    }
    let thin = |c: char| c == '-' || c == '_';
    let mut rest = text;
    while !rest.is_empty() {
        let is_thin = rest.starts_with(thin);
        let end = rest
            .find(|c: char| thin(c) != is_thin)
            .unwrap_or(rest.len());
        let font = if is_thin {
            crate::ui::theme::arial(dz(PATH_SIZE))
        } else {
            design::myriad(PATH_SIZE)
        };
        job.append(&rest[..end], 0.0, format(font));
        rest = &rest[end..];
    }
}

fn info_line(kit: &mut Kit, a: &Action) {
    let mut job = LayoutJob::default();
    for (i, part) in a.info.iter().enumerate() {
        if i > 0 {
            info_part(&mut job, "  ·  ", a.info_color);
        }
        info_part(&mut job, part, a.info_color);
    }
    let g = kit.ui.ctx().fonts_mut(|f| f.layout_job(job));
    let (x, y) = (dz(144.0), dz(INFO_Y) - g.size().y / 2.0);
    kit.clipped(x, y - 2.0, dz(1282.0) - x, g.size().y + 4.0, |kit| {
        kit.put(x, y, g);
    });
}

/// PLAY and CHECK FOR UPDATES, or a running job's progress in their place.
fn buttons(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context, a: Action) {
    if let Some((id, label, fraction)) = &a.job {
        job_row(d, kit, id, label, *fraction);
        return;
    }
    let dim = |on: bool| {
        if on {
            Color32::WHITE
        } else {
            Color32::from_gray(140)
        }
    };

    let (resp, t, pressed) = kit.hot_shape("play-main", PLAY_AREA, &PLAY_SHAPE, a.enabled, &a.tip);
    if a.label == "PLAY" {
        kit.image_tinted("play_button.png", PLAY_IMG, dim(a.enabled));
    } else {
        kit.image_tinted("play_button_blank.png", PLAY_IMG, dim(a.enabled));
        play_label(kit, a.label, a.enabled);
    }
    kit.shape_veil(&PLAY_SHAPE, t, pressed);
    if resp.clicked {
        match a.main {
            Main::Play => try_start(d, ctx),
            Main::Stop => {
                if let Some(mut c) = d.child.take() {
                    let _ = c.kill();
                }
            }
            Main::Install(e) | Main::Reinstall(e) => versions::install(d, ctx, e),
            Main::Patch(id) => {
                setup::patch(d, ctx, &id, crate::core::launcher::patch::Source::Discord)
            }
            Main::SetUpRevive => setup::revive(d, ctx),
            Main::QuestConnect => d.check_quest(ctx, true),
            Main::QuestInstall => setup::ask_quest_install(d),
            Main::QuestPlay => quest_launch(d, ctx),
            Main::Nothing => {}
        }
    }

    let tip = match a.update {
        Update::Quest => "Copy the latest game files to your Quest",
        _ => "Download any changed game files",
    };
    let (resp, t, pressed) =
        kit.hot_shape("update", UPDATE_AREA, &UPDATE_SHAPE, a.update_enabled, tip);
    let img = if a.update_alert {
        "update_button_alert.png"
    } else {
        "update_button.png"
    };
    kit.image_tinted(img, UPDATE_IMG, dim(a.update_enabled));
    kit.shape_veil(&UPDATE_SHAPE, t, pressed);
    if resp.clicked {
        match a.update {
            Update::Pc(v) => versions::update(d, ctx, v),
            Update::Quest => setup::quest_update(d, ctx),
            Update::Nothing => {}
        }
    }
}

/// A label on the blank PLAY button, as big as the image's own "PLAY" where it fits.
fn play_label(kit: &Kit, label: &str, enabled: bool) {
    let probe = design::conthrax(40.0);
    let size = 40.0 * dz(PLAY_WORD_W) / kit.text_width("PLAY", probe.clone());
    let fit = dz(PLAY_LABEL.w) / kit.text_width(label, probe);
    let font = design::conthrax(size.min(40.0 * fit));
    let r = kit.drect(PLAY_LABEL);
    let (fg, shadow) = if enabled {
        (
            Color32::from_rgb(214, 250, 206),
            Color32::from_rgb(12, 120, 12),
        )
    } else {
        (Color32::from_gray(200), Color32::from_gray(90))
    };
    let p = kit.ui.painter();
    p.text(
        r.center() + egui::vec2(0.0, 1.0),
        egui::Align2::CENTER_CENTER,
        label,
        font.clone(),
        shadow,
    );
    p.text(r.center(), egui::Align2::CENTER_CENTER, label, font, fg);
}

/// A running job in place of the buttons: its progress and a small Cancel.
fn job_row(d: &mut Dashboard, kit: &mut Kit, id: &str, label: &str, f: Option<f32>) {
    let r = kit.drect(JOB_ROW);
    let (x, y, w) = (dz(JOB_ROW.x), dz(JOB_ROW.y), dz(JOB_ROW.w));
    let mid = y + r.height() / 2.0;
    kit.progress(x, mid - 17.0, w - 132.0, f, label);
    if kit
        .flat_button(
            "job-cancel",
            Variant::Ghost,
            None,
            "Cancel",
            x + w - 120.0,
            mid - style::SMALL / 2.0,
            120.0,
            style::SMALL,
            true,
            "Stop this job",
        )
        .clicked
    {
        d.cancel_job(id);
    }
}

/// The PCVR | QUEST switch: the image shows the side in use, each half picks its side.
fn switch(d: &mut Dashboard, kit: &mut Kit) {
    let img = match d.play_platform {
        Platform::Pc => "hardware_pc.png",
        Platform::Quest => "hardware_quest.png",
    };
    kit.image_d(img, SWITCH_IMG);
    let sides = [
        (Platform::Pc, SWITCH_PC, "Echo VR on this PC"),
        (
            Platform::Quest,
            SWITCH_QUEST,
            "Echo VR on your Quest, over USB",
        ),
    ];
    for (i, (p, area, tip)) in sides.into_iter().enumerate() {
        let r = kit.drect(area);
        let (resp, t, _) = kit.hot(&format!("side-{i}"), r, d.play_platform != p, tip);
        if t > 0.01 {
            kit.ui.painter().rect_filled(
                r.shrink(1.0),
                dz(8.0),
                Color32::from_white_alpha((28.0 * t) as u8),
            );
        }
        if resp.clicked {
            d.play_platform = p;
        }
    }
}

// ---- Community News ----

fn news(d: &mut Dashboard, kit: &mut Kit) {
    kit.image_d(
        "news_header.png",
        Dr::new(NEWS.x, NEWS.y, NEWS.w, NEWS_HEADER_H),
    );
    let banner = Dr::new(
        NEWS.x,
        NEWS.y + NEWS_HEADER_H,
        NEWS.w,
        NEWS.h - NEWS_HEADER_H,
    );
    let news = d.feed.news.clone();
    let main = news.as_ref().and_then(|n| n.slots.main.clone());
    let tex = main
        .as_ref()
        .and_then(|m| d.feed.texture(m.image.as_deref()))
        .cloned();
    match (&news, tex) {
        // Nothing loaded (yet), and snapshots: the design's banner.
        (None, _) => kit.image_d("news_fallback.jpg", banner),
        (Some(_), None) if d.demo && !d.feed_live => kit.image_d("news_fallback.jpg", banner),
        (Some(_), Some(tex)) => {
            let r = kit.drect(banner);
            kit.texture_cover(&tex, r, dz(4.0));
            kit.gradient_frame(
                r,
                dz(4.0),
                dz(2.0),
                Color32::from_rgb(120, 60, 200),
                design::RIM_BOTTOM,
            );
        }
        // A message without a picture (or still downloading it): its title on the panel.
        (Some(_), None) => {
            kit.image_d("card_bg.png", banner);
            if let Some(m) = &main {
                let g = kit.spaced_galley(
                    &m.title.to_uppercase(),
                    design::conthrax(40.0),
                    design::TEXT,
                    dz(4.0),
                    false,
                );
                kit.clipped(
                    dz(banner.x + 40.0),
                    dz(banner.y),
                    dz(banner.w - 80.0),
                    dz(banner.h),
                    |kit| {
                        kit.put(dz(banner.x + 50.0), dz(banner.y + 60.0), g);
                    },
                );
            }
        }
    }
    let link = match (&news, &main) {
        (None, _) => Some((
            "How to play".to_string(),
            crate::core::oauth::INVITE_URL.to_string(),
        )),
        (_, Some(m)) if !m.link_label.is_empty() && !m.link_url.is_empty() => {
            Some((m.link_label.clone(), m.link_url.clone()))
        }
        _ => None,
    };
    if let Some((label, url)) = link {
        let g = kit.spaced_galley(
            &label.to_uppercase(),
            design::conthrax(21.0),
            design::TEXT,
            dz(5.5),
            true,
        );
        let (right, base) = NEWS_LINK;
        let r = kit.put(dz(right) - g.size().x, dz(base) - g.size().y, g);
        if kit.click_area("news-link", r, &url) {
            crate::core::platform::open_url(&url);
        }
    }

    let (main_card, community_card) = cards(d, news.as_ref().map(|n| n.slots.clone()), main);
    card(kit, 0, CARDS[0], &main_card, false);
    card(kit, 1, CARDS[1], &community_card, true);
}

/// The two cards' contents: the `main` message, and the `community` one (or a pointer to
/// the Discord when it isn't set).
fn cards(
    d: &Dashboard,
    slots: Option<crate::core::launcher::feed::Slots>,
    main: Option<NewsItem>,
) -> (NewsItem, NewsItem) {
    let text = |title: &str, body: &str| NewsItem {
        title: title.into(),
        body: body.into(),
        ..Default::default()
    };
    let main = match (main, &slots) {
        (Some(m), _) => m,
        (None, Some(_)) => text("Community news", "Nothing new right now."),
        (None, None) if d.feed.news_failed => text(
            "Community news",
            "The news couldn't be loaded. Check your internet connection.",
        ),
        (None, None) => text("Community news", "Loading the latest news..."),
    };
    let community = slots.and_then(|s| s.community).unwrap_or_else(|| NewsItem {
        title: "Community".into(),
        body:
            "Matches, events, help and the latest builds: the Echo VR community meets on Discord."
                .into(),
        link_label: "Join the Discord".into(),
        link_url: crate::core::oauth::INVITE_URL.into(),
        ..Default::default()
    });
    (main, community)
}

fn card_look() -> Look {
    Look {
        font: design::din(15.8),
        bold: design::din(15.8),
        mono: egui::FontId::monospace(dz(15.0)),
        small: design::din(14.0),
        color: design::TEXT,
        strong: design::TEXT,
        subtle: design::BODY,
        link: design::TEXT,
        chip: Color32::from_white_alpha(18),
        chip_rim: Color32::TRANSPARENT,
        line_gap: 0.0,
        paragraph_gap: dz(18.0),
        bullet_indent: dz(16.0),
        uppercase: true,
        dash_bullets: true,
    }
}

/// A news card: the title in Conthrax, the text in DIN caps, and its link (when asked).
fn card(kit: &mut Kit, i: usize, r: Dr, item: &NewsItem, with_link: bool) {
    kit.image_d("card_bg.png", r);
    kit.gradient_frame(
        kit.drect(r),
        dz(6.0),
        dz(2.0),
        design::RIM_TOP,
        design::RIM_BOTTOM,
    );
    let inner = r.shrink(22.0);
    let title = kit.spaced_galley(
        &item.title.to_uppercase(),
        design::conthrax(24.0),
        design::TEXT,
        dz(1.8),
        false,
    );
    let (x, w) = (dz(inner.x), dz(inner.w));
    let mut y = dz(r.y + 20.0);
    kit.clipped(x, y, w, title.size().y, |kit| {
        kit.put(x, y, title.clone());
    });
    y = dz(r.y + 72.0);
    let look = card_look();
    let bottom = dz(inner.bottom());
    let body_h = kit.clipped(x, y, w, bottom - y, |kit| {
        markdown::draw(kit, x, y, w, &item.body, &look)
    });
    if with_link && !item.link_label.is_empty() && !item.link_url.is_empty() {
        let g = kit.spaced_galley(
            &item.link_label.to_uppercase(),
            design::din(16.5),
            design::TEXT,
            dz(0.5),
            true,
        );
        let ly = (y + body_h + look.paragraph_gap).min(bottom - g.size().y);
        let lr = kit.put(x, ly, g);
        if kit.click_area(&format!("card-link-{i}"), lr, &item.link_url) {
            crate::core::platform::open_url(&item.link_url);
        }
    }
}

// ---- starting ----

fn quest_launch(d: &mut Dashboard, ctx: &egui::Context) {
    d.quest_busy = true;
    d.worker.spawn(ctx, |tx| {
        tx.send(Msg::QuestAction(quest::launch().map_err(|e| {
            UiError::from_anyhow(&e, "Couldn't start Echo VR")
        })))
    });
}

fn try_start(d: &mut Dashboard, ctx: &egui::Context) {
    match launch::preflight(&d.state.profile) {
        Some(w) => d.dialogs.confirm(
            LAUNCH_ANYWAY,
            "Launch Echo VR",
            &format!("{w}\n\nLaunch anyway?"),
            DlgIcon::Warning,
        ),
        None => start(d, ctx),
    }
}

fn start(d: &mut Dashboard, ctx: &egui::Context) {
    let Target::Installed(v) = d.target() else {
        return;
    };
    let exe = paths::exe_path(&v.root);
    if !exe.is_file() {
        d.dialogs.error(
            "Echo VR not found",
            &format!("echovr.exe is missing in {}.\nRepair or reinstall this version on the Versions page.", v.root),
            Default::default(),
        );
        return;
    }
    let revive_dir = if d.state.profile.runtime == Runtime::Revive {
        revive::find_revive_dir()
    } else {
        None
    };
    // No lobby: joining one needs the lobby field back on this page first.
    let result = launch::build(&d.state.profile, &exe, revive_dir.as_deref(), None)
        .and_then(|c| launch::spawn(&c));
    match result {
        Ok(child) => {
            d.child = Some(child);
            if d.state.minimize_on_launch {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
        }
        Err(e) => d.dialogs.error(
            "Couldn't start Echo VR",
            &format!("{e:#}"),
            Default::default(),
        ),
    }
}
