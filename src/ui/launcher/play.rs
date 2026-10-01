//! Play page, as the design concept has it: the ECHO VR logo over an info line, PLAY,
//! CHECK FOR UPDATES and the PCVR|QUEST switch, Community News (a banner and two cards)
//! and SERVER INFO on the right. PLAY does what the selected version needs (patch, set up
//! SteamVR, play, stop); on the Quest side it connects or starts Echo VR on the headset.
//! With nothing installed it is grey and opens the Install page. While a job runs on the
//! installed game, PLAY fills up with its progress and CHECK FOR UPDATES becomes CANCEL.

use egui::Color32;

use super::hero::{self, Face, InfoLine, JobView, PathClick, Row, Side};
use super::{server_info, setup, versions, Dashboard, Msg, Page};
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
use crate::ui::style::{self, Icon};
use crate::ui::widgets::MenuItem;
use egui::pos2;

const LAUNCH_ANYWAY: &str = "launch-anyway";

/// The info line's right limit (design pixels).
const INFO_RIGHT: f32 = 1282.0;
const NEWS: Dr = Dr::new(138.0, 350.0, 1144.0, 421.0);
/// news_header.png is the top 75 of CommunityNewsTab's 697 px.
const NEWS_HEADER_H: f32 = 421.0 * 75.0 / 697.0;
/// The news link's right end and baseline, at the banner's bottom right.
const NEWS_LINK: (f32, f32) = (1226.0, 727.0);
/// The design banner's lettering (news_fallback_text.png): its left edge, width and
/// height, at the scale of its background in a 1280×720 window.
const NEWS_LETTERING: (f32, f32, f32) = (NEWS.x + 48.0, 439.0, 275.0);
const CARDS: [Dr; 2] = [
    Dr::new(137.0, 800.0, 555.0, 248.0),
    Dr::new(728.0, 800.0, 555.0, 248.0),
];
/// The version picker right of the PCVR|QUEST switch: its body (as tall as the switch's)
/// and its caption, level with the switch's.
const PICKER: Dr = Dr::new(878.0, 265.0, 260.0, 43.0);
const PICKER_CAPTION_Y: f32 = 248.4;
pub(super) const VERSION_MENU: &str = "version-picker";
const PICKER_RADIUS: f32 = 8.0;
/// The menu reaches from the picker to the column's right edge.
const PICKER_MENU_W: f32 = INFO_RIGHT - PICKER.x;
/// The switch's PCVR / QUEST captions: grey DMCAPS, 8 px tall capitals.
const PICKER_CAPTION_SIZE: f32 = 11.5;
const CAPTION: Color32 = Color32::from_gray(118);
const CAPTION_ASCENT: f32 = 0.2;

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    // Look for a headset quietly once, so the Quest chip has something to say.
    if !d.quest_auto_checked && !d.demo && !kit.ghost {
        d.quest_auto_checked = true;
        if d.quest_conn.status.is_none() && !d.quest_conn.checking {
            d.check_quest(ctx, false);
        }
    }
    if !kit.ghost && d.dialogs.take(LAUNCH_ANYWAY).is_some_and(|a| a.is_yes()) {
        start(d, ctx);
    }
    kit.image_d("logo_echovr.png", hero::LOGO);
    let mut a = match d.platform {
        Platform::Pc => pc_action(d),
        Platform::Quest => quest_action(d),
    };
    if a.job.is_none() {
        explain_disabled(d, &mut a);
    }
    // Starting: look again soon in case the game's process ends before it shows up.
    if d.ours() && !d.game().is_running() {
        ctx.request_repaint_after(std::time::Duration::from_millis(500));
    }
    hero::info_line(d, kit, "info-path", &a.line);
    // PLAY keeps the concept's size; longer labels are set smaller.
    match a.job.take() {
        Some(job) => {
            if hero::job_row(kit, "play", 0.0, &job) {
                d.cancel_job(&job.id);
            }
        }
        None => buttons(d, kit, ctx, a),
    }
    hero::switch(kit, "side", 0.0, &mut d.platform);
    if d.platform == Platform::Pc {
        version_picker(d, kit);
    }
    news(d, kit);
    server_info::at_right(kit, |k| server_info::show(d, k));
}

/// What PLAY does.
enum Main {
    Play,
    Stop,
    Patch(String),
    SetUpRevive,
    QuestConnect,
    QuestPlay,
    /// Nothing to play yet: open the Install page.
    ToInstall,
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
    line: InfoLine,
    /// PLAY's label; "PLAY" is the image's own lettering.
    label: &'static str,
    /// Nothing installed: PLAY in greys.
    grey: bool,
    main: Main,
    enabled: bool,
    tip: String,
    /// A job on what PLAY would start: shown in the buttons.
    job: Option<JobView>,
    update: Update,
    update_enabled: bool,
    update_tip: String,
    /// The last update failed: the button shows its orange "!".
    update_alert: bool,
}

impl Action {
    fn new() -> Action {
        Action {
            line: InfoLine::new(INFO_RIGHT),
            label: "PLAY",
            grey: false,
            main: Main::Nothing,
            enabled: false,
            tip: String::new(),
            job: None,
            update: Update::Nothing,
            update_enabled: false,
            update_tip: "Download any changed game files".into(),
            update_alert: false,
        }
    }

    /// Nothing to play: a grey PLAY that opens the Install page, and one word on the info
    /// line (`state`, or the install's progress while one runs).
    fn not_installed(&mut self, state: &str, job: Option<&JobView>) {
        self.grey = true;
        (self.main, self.enabled) = (Main::ToInstall, true);
        self.tip = "Echo VR isn't installed yet. Click to install it".into();
        self.update_tip = "Install Echo VR first".into();
        self.line.parts = match job.filter(|j| j.installs()) {
            Some(j) => vec![
                "Installing".into(),
                j.fraction
                    .map_or_else(|| j.step(), |f| format!("{:.0}%", f * 100.0)),
            ],
            None => vec![state.into()],
        };
    }
}

pub(super) fn gb(bytes: u64) -> String {
    let g = bytes as f64 / 1e9;
    if g >= 10.0 || (g - g.round()).abs() < 0.05 {
        format!("{g:.0}GB")
    } else {
        format!("{g:.1}GB")
    }
}

/// Why PLAY and CHECK FOR UPDATES can't be used right now, in their tooltips.
fn explain_disabled(d: &Dashboard, a: &mut Action) {
    let busy = d
        .jobs
        .values()
        .next()
        .map(|j| format!("Busy: {}. Wait until it's done.", j.title));
    if !a.enabled && !matches!(a.main, Main::Nothing) {
        if let Some(b) = &busy {
            a.tip = b.clone();
        }
    }
    if !a.update_enabled {
        a.update_tip = match (&busy, &a.update) {
            (Some(b), _) => b.clone(),
            (None, Update::Nothing) if d.platform == Platform::Pc => "Install Echo VR first".into(),
            (None, Update::Quest) if d.quest_conn.status != Some(Status::Ready) => {
                "Connect your Quest first".into()
            }
            (None, Update::Quest) => "Install Echo VR on your Quest first".into(),
            (None, _) if d.game().is_running() => "Close Echo VR first".into(),
            (None, _) => a.update_tip.clone(),
        };
    }
}

// ---- PC ----

fn pc_action(d: &mut Dashboard) -> Action {
    let target = d.target();
    let id = match &target {
        Target::Installed(v) | Target::Missing(v) => Some(v.id.clone()),
        Target::Available(e) => Some(e.id.clone()),
        Target::None => None,
    };
    let job = id
        .and_then(|id| hero::job_view(d, &id))
        .or_else(|| hero::job_view(d, setup::REVIVE_JOB));
    let mut a = Action::new();
    let needs_revive = d.state.profile.runtime == Runtime::Revive
        && (cfg!(windows) || d.demo)
        && d.revive_missing();
    let running = d.game().is_running();
    let ours = d.ours();
    match target {
        Target::Installed(v) => {
            a.job = job;
            let size = v.catalog_id.as_ref().and_then(|cid| {
                let c = d.catalog.as_ref()?;
                c.pc().find(|e| &e.id == cid)?.size.map(gb)
            });
            let state = if running {
                "Running"
            } else if ours {
                "Starting"
            } else if setup::needs_patch(d, &v) {
                "Needs the licence patch"
            } else if needs_revive {
                "SteamVR is not set up"
            } else {
                "Installed"
            };
            a.line.parts.push(state.into());
            a.line.parts.push(v.name.clone());
            a.line.parts.extend(size);
            a.line.path = Some(v.root.clone());
            a.line.path_click = PathClick::Open;
            if let Some(job) = &a.job {
                a.line.parts[0] = job_state(job);
                a.line.parts.push(job.step());
            }
            if running && ours {
                (a.label, a.main, a.enabled, a.grey) = ("STOP", Main::Stop, true, true);
                a.tip = "Close Echo VR".into();
            } else if running {
                a.label = "RUNNING";
                a.tip = "Echo VR was started outside the launcher.".into();
            } else if ours {
                // Clicked: the grey PLAY until the game shows up (or its process ends).
                a.tip = "Starting Echo VR…".into();
            } else if setup::needs_patch(d, &v) {
                (a.label, a.main) = ("PATCH", Main::Patch(v.id.clone()));
                a.enabled = !d.any_job();
                a.tip = "New players need a personal licence patch: authorize with Discord to get yours.".into();
            } else if needs_revive {
                (a.label, a.main) = ("SET UP", Main::SetUpRevive);
                a.enabled = !d.any_job();
                a.tip = "Set up SteamVR: installs Revive, which runs Echo VR on SteamVR (asks for administrator rights)".into();
            } else {
                (a.main, a.enabled) = (Main::Play, true);
                a.tip = "Start Echo VR".into();
            }
            a.update_enabled = !running && !ours && !d.any_job();
            if ours && !running {
                a.update_tip = "Echo VR is starting".into();
            }
            a.update_alert = d
                .update_note
                .get(&v.id)
                .is_some_and(|n| n.contains("failed"));
            a.update = Update::Pc(v);
        }
        Target::Missing(_) => {
            a.not_installed("Game files missing", job.as_ref());
            if !job.as_ref().is_some_and(JobView::installs) {
                a.line.color = design::DANGER;
                a.tip =
                    "The game files are gone. Click to install Echo VR again or add its new folder"
                        .into();
            }
        }
        Target::Available(_) | Target::None => a.not_installed("Not installed", job.as_ref()),
    }
    a
}

// ---- Quest ----

fn quest_action(d: &mut Dashboard) -> Action {
    let ready = d.quest_conn.status == Some(Status::Ready);
    let installed = ready && d.quest_info.as_ref().is_some_and(|i| i.installed);
    let known = ready && d.quest_info.is_some();
    let busy = d.quest_conn.checking || d.quest_busy || d.any_job();
    let job = hero::job_view(d, setup::QUEST_JOB);
    let mut a = Action::new();
    a.line.parts = quest_info(d);
    if job.as_ref().is_some_and(JobView::installs) || (known && !installed) {
        a.not_installed("Not installed on this Quest", job.as_ref());
        a.tip = "Echo VR isn't on your Quest yet. Click to install it".into();
        a.line
            .parts
            .extend(d.quest_info.as_ref().and_then(|i| i.device.clone()));
    } else if installed {
        a.job = job;
        if let Some(job) = &a.job {
            a.line.parts[0] = job_state(job);
            a.line.parts.push(job.step());
        }
        (a.main, a.enabled) = (Main::QuestPlay, !d.quest_busy);
        a.tip = "Start Echo VR on the headset".into();
    } else {
        a.label = "CONNECT";
        a.main = Main::QuestConnect;
        a.enabled = !d.quest_conn.checking && !d.quest_busy;
        a.tip = "Look for your Quest over USB".into();
    }
    a.update = Update::Quest;
    a.update_enabled = installed && !busy;
    a.update_tip = "Copy the latest game files to your Quest".into();
    a
}

/// What the headset's connection and install are, for an info line.
pub(super) fn quest_info(d: &Dashboard) -> Vec<String> {
    let device = d.quest_info.as_ref().and_then(|i| i.device.clone());
    match (d.quest_conn.checking, d.quest_conn.status) {
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
    }
}

/// What a running job is doing, as the info line's first word.
pub(super) fn job_state(job: &JobView) -> String {
    use super::JobKind;
    match job.kind {
        JobKind::Install | JobKind::QuestInstall => "Installing".into(),
        JobKind::Update => "Updating".into(),
        JobKind::Verify => "Verifying".into(),
        JobKind::Patch => "Patching".into(),
        JobKind::Background => "Converting".into(),
        JobKind::Revive | JobKind::QuestUpdate => job.title.clone(),
    }
}

// ---- the buttons ----

/// PLAY and CHECK FOR UPDATES.
fn buttons(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context, a: Action) {
    let face = match a.label {
        "RUNNING" => Face::Running,
        label => Face::Label(label),
    };
    let row = Row {
        face,
        grey: a.grey,
        enabled: a.enabled,
        tip: &a.tip,
        side: Side::Updates {
            alert: a.update_alert,
        },
        side_enabled: a.update_enabled,
        side_tip: &a.update_tip,
    };
    let (main, update) = hero::row(kit, "play", 0.0, &row);
    if main {
        match a.main {
            Main::Play => try_start(d, ctx),
            Main::Stop => {
                if let Some(mut c) = d.child.take() {
                    let _ = c.kill();
                }
            }
            Main::Patch(id) => {
                setup::patch(d, ctx, &id, crate::core::launcher::patch::Source::Discord)
            }
            Main::SetUpRevive => setup::revive(d, ctx),
            Main::QuestConnect => d.check_quest(ctx, true),
            Main::QuestPlay => quest_launch(d, ctx),
            Main::ToInstall => {
                // The Install page opens on the version picked here.
                d.install_pick = match d.target() {
                    Target::Available(e) => Some(e.id),
                    Target::Missing(v) => v.catalog_id,
                    _ => None,
                };
                d.page = Page::Install;
            }
            Main::Nothing => {}
        }
    }
    if update {
        match a.update {
            Update::Pc(v) => versions::update(d, ctx, v),
            Update::Quest => setup::quest_update(d, ctx),
            Update::Nothing => {}
        }
    }
}

// ---- the version picker ----

/// VERSION, right of the switch: the version PLAY starts, and a menu to switch to
/// another. Installed versions come first, then the catalogue's others: picking one of
/// those greys PLAY, which then opens the Install page on it.
fn version_picker(d: &mut Dashboard, kit: &mut Kit) {
    let installed = d.state.versions.clone();
    let available: Vec<VersionEntry> = d
        .catalog
        .as_ref()
        .map(|c| {
            c.pc()
                .filter(|e| d.state.installed_from(&e.id).is_none())
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if installed.is_empty() && available.is_empty() {
        return;
    }
    let (current_id, current_name) = match d.target() {
        Target::Installed(v) | Target::Missing(v) => (Some(v.id), v.name),
        Target::Available(e) => (Some(e.id), e.name),
        Target::None => (None, "Choose a version".to_string()),
    };
    let running = d.game().is_running() || d.ours();
    let busy = d
        .jobs
        .values()
        .next()
        .map(|j| format!("Busy: {}. Wait until it's done.", j.title));
    let (enabled, tip) = match (running, busy) {
        (true, _) => (false, "Close Echo VR to switch versions".to_string()),
        (false, Some(b)) => (false, b),
        (false, None) => (true, "The version PLAY starts: click to switch".to_string()),
    };

    let caption = kit.spaced_galley(
        "VERSION",
        design::din(PICKER_CAPTION_SIZE),
        CAPTION,
        dz(0.9),
        false,
    );
    let (cx, cy) = (dz(PICKER.x), dz(PICKER_CAPTION_Y) - caption_top(kit));
    kit.put(cx, cy, caption);

    let r = kit.drect(PICKER);
    let (resp, t, pressed) = kit.hot(VERSION_MENU, r, enabled, &tip);
    let open = enabled && kit.menu_open(VERSION_MENU);
    let fill = if pressed {
        Color32::from_gray(12)
    } else {
        style::mix(design::DARK, Color32::from_gray(40), t)
    };
    let radius = dz(PICKER_RADIUS);
    kit.ui.painter().rect_filled(r, radius, fill);
    if open {
        kit.ui.painter().rect_stroke(
            r,
            radius,
            egui::Stroke::new(dz(2.0), design::BLUE),
            egui::StrokeKind::Inside,
        );
    }
    let fg = if enabled {
        design::TEXT
    } else {
        Color32::from_gray(150)
    };
    let chevron = dz(15.0);
    let pad = dz(18.0);
    let g = kit.spaced_fit(
        &current_name.to_uppercase(),
        design::din(17.0),
        fg,
        dz(1.2),
        false,
        r.width() - 2.0 * pad - chevron - dz(10.0),
    );
    let gy = r.center().y - g.size().y / 2.0;
    kit.ui.painter().galley(pos2(r.min.x + pad, gy), g, fg);
    style::icon_at(
        kit.ui.painter(),
        Icon::ChevronDown,
        pos2(r.max.x - pad - chevron, r.center().y - chevron / 2.0),
        chevron,
        fg,
    );
    if resp.clicked {
        kit.toggle_menu(VERSION_MENU);
        return;
    }
    if !enabled {
        return;
    }

    let checked = |id: &str| current_id.as_deref() == Some(id);
    let mut items: Vec<MenuItem> = installed
        .iter()
        .map(|v| {
            let present = d.demo || paths::has_echo_install(&v.root);
            let (detail, detail_color) = if !present {
                ("Files missing".to_string(), design::DANGER)
            } else if v.external {
                ("Existing folder".to_string(), design::GREY)
            } else {
                let size = v.catalog_id.as_ref().and_then(|cid| {
                    let c = d.catalog.as_ref()?;
                    c.pc().find(|e| &e.id == cid)?.size.map(gb)
                });
                (size.unwrap_or_default(), design::GREY)
            };
            MenuItem::Pick {
                label: v.name.clone(),
                detail,
                detail_color,
                checked: checked(&v.id),
                tip: v.root.clone(),
            }
        })
        .collect();
    items.extend(available.iter().map(|e| MenuItem::Pick {
        label: e.name.clone(),
        detail: match e.size {
            Some(s) => format!("Not installed  ·  {}", gb(s)),
            None => "Not installed".into(),
        },
        detail_color: design::GREY,
        checked: checked(&e.id),
        tip: "Not installed yet: PLAY takes you to the Install page".into(),
    }));
    items.push(MenuItem::Divider);
    items.push(MenuItem::row(
        "Install another version…",
        "Open the Install page",
    ));
    let w = r.width().max(dz(PICKER_MENU_W));
    let anchor = egui::Rect::from_min_size(r.min, egui::vec2(w, r.height()));
    match kit.menu_at(VERSION_MENU, anchor, &items) {
        Some(i) if i < installed.len() + available.len() => {
            let id = match installed.get(i) {
                Some(v) => v.id.clone(),
                None => available[i - installed.len()].id.clone(),
            };
            d.state.selected = Some(id);
            d.save();
        }
        Some(_) => d.page = Page::Install,
        None => {}
    }
}

/// How far below its galley's top a DMCAPS capital starts, at the caption's size.
fn caption_top(kit: &Kit) -> f32 {
    let g = kit.spaced_galley("V", design::din(PICKER_CAPTION_SIZE), CAPTION, 0.0, false);
    // DMCAPS' ascent sits above its capitals; measured once from the switch's labels.
    g.size().y * CAPTION_ASCENT
}

// ---- Community News ----

fn news(d: &mut Dashboard, kit: &mut Kit) {
    let dx = kit.dx();
    // A wider window: the header strip stretches (its title stays), the design's banner
    // background fills the wider card, and a downloaded banner keeps its size, centred.
    kit.image_wider(
        "news_header.png",
        Dr::new(NEWS.x, NEWS.y, NEWS.w, NEWS_HEADER_H),
        dx,
        (420.0, 8.0),
        Color32::WHITE,
    );
    let area = Dr::new(
        NEWS.x,
        NEWS.y + NEWS_HEADER_H,
        NEWS.w + dx,
        NEWS.h - NEWS_HEADER_H,
    );
    let banner = Dr::new(area.x + dx / 2.0, area.y, NEWS.w, area.h);
    let news = d.feed.news.clone();
    let main = news.as_ref().and_then(|n| n.slots.main.clone());
    let tex = main
        .as_ref()
        .and_then(|m| d.feed.texture(m.image.as_deref()))
        .cloned();
    // Nothing loaded (yet), and snapshots: the design's banner, with its HOW TO PLAY.
    let fallback = news.is_none() || (tex.is_none() && d.demo && !d.feed_live);
    match tex {
        // The background covers the card (cropped top and bottom when it's wider); the
        // lettering stays at its left.
        _ if fallback => {
            kit.image_cover("news_fallback_bg.jpg", area, dz(4.0));
            let (x, w, h) = NEWS_LETTERING;
            let y = area.y + (area.h - h) / 2.0;
            kit.image_d("news_fallback_text.png", Dr::new(x, y, w, h));
            banner_rim(kit, area);
        }
        Some(tex) => {
            if dx > 0.0 {
                kit.image_d("card_bg.png", area);
            }
            kit.texture_cover(&tex, kit.drect(banner), dz(4.0));
            banner_rim(kit, area);
        }
        // A message without a picture (or still downloading it): its title on the panel.
        None => {
            kit.image_d("card_bg.png", area);
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
        _ if fallback => Some((
            "How to play".to_string(),
            crate::core::oauth::INVITE_URL.to_string(),
        )),
        (_, Some(m)) if !m.link_label.is_empty() && !m.link_url.is_empty() => {
            Some((m.link_label.clone(), m.link_url.clone()))
        }
        _ => None,
    };
    if let Some((label, url)) = link {
        let (right, base) = NEWS_LINK;
        let g = kit.spaced_fit(
            &label.to_uppercase(),
            design::conthrax(21.0),
            design::TEXT,
            dz(5.5),
            true,
            dz(700.0),
        );
        let r = kit.put(dz(right + dx) - g.size().x, dz(base) - g.size().y, g);
        if kit.click_area("news-link", r, &url) {
            crate::core::platform::open_url(&url);
        }
    }

    let (main_card, community_card) = cards(news.as_ref().map(|n| n.slots.clone()), main);
    // Side by side, sharing the extra width; taller in a taller window.
    let (half, dy) = (dx / 2.0, kit.dy());
    card(kit, 0, CARDS[0].wider(half).taller(dy), &main_card, false);
    let second = CARDS[1].moved(half).wider(half).taller(dy);
    card(kit, 1, second, &community_card, true);
}

/// The rim around a picture banner: violet at the top to pink at the bottom.
fn banner_rim(kit: &Kit, area: Dr) {
    kit.gradient_frame(
        kit.drect(area),
        dz(4.0),
        dz(2.0),
        Color32::from_rgb(120, 60, 200),
        design::RIM_BOTTOM,
    );
}

/// The two cards' contents: the `main` message, and the `community` one (or a pointer to
/// the Discord when it isn't set).
fn cards(
    slots: Option<crate::core::launcher::feed::Slots>,
    main: Option<NewsItem>,
) -> (NewsItem, NewsItem) {
    let text = |title: &str, body: &str| NewsItem {
        title: title.into(),
        body: body.into(),
        ..Default::default()
    };
    let main = main.unwrap_or_else(|| {
        text(
            "Welcome",
            "Your launcher for Echo VR, on PC and on your Quest.\n\n\
             - Install, update and play from one place\n\
             - Switch between PCVR and Quest next to PLAY\n\
             - Server status and this week's best on the right\n\n\
             Community news appears here when there is some.",
        )
    });
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
    let (x, y, w, bottom) = hero::card_frame(kit, r, &item.title);
    let look = card_look();
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
            &format!(
                "echovr.exe is missing in {}.\nRepair or reinstall it on the Install page.",
                v.root
            ),
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
