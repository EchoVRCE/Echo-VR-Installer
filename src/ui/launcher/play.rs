//! Play page: "Echo VR" over the game art, a left column with the version and headset
//! pickers, a full-width PLAY and the launch options (shown with a checkbox), and a row of
//! cards (join lobby, updates, Quest). The Quest side starts Echo on the headset.

use egui::Rect;

use super::{setup, versions, Dashboard, Msg, Page, CW, X0};
use crate::core::adb::devices::Status;
use crate::core::error::UiError;
use crate::core::launcher::catalog::{Platform, VersionEntry};
use crate::core::launcher::store::{InstalledVersion, Runtime, Target};
use crate::core::launcher::{launch, quest};
use crate::core::{paths, revive};
use crate::ui::dialogs::Icon as DlgIcon;
use crate::ui::kit::Kit;
use crate::ui::style::{self, Icon, MenuItem, Variant};

const LAUNCH_ANYWAY: &str = "launch-anyway";
/// The version picker's menu key (snapshots open it).
pub(super) const VERSION_KEY: &str = "version";

// The left column, in fixed slots so nothing moves when the state changes.
const COL_W: f32 = 520.0;
const VERSION_W: f32 = 280.0;
const TABS_Y: f32 = 60.0;
const TITLE_Y: f32 = 100.0;
const STATUS_Y: f32 = 150.0;
const LABEL_Y: f32 = 188.0;
const PICK_Y: f32 = 206.0;
const PLAY_Y: f32 = 256.0;
const HINT_Y: f32 = 316.0;
const OPTS_Y: f32 = 340.0;
const OPTS_CARD_Y: f32 = 372.0;
const OPTS_CARD_H: f32 = 102.0;
/// The Quest side's row of smaller buttons under its main button.
const SECOND_Y: f32 = 318.0;
const CARDS_Y: f32 = 508.0;
const CARD_H: f32 = 168.0;

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    let sides = [
        (Platform::Pc, "PC", "Echo VR on this PC"),
        (Platform::Quest, "Quest", "Echo VR on your Quest, over USB"),
    ];
    for (i, (p, label, tip)) in sides.into_iter().enumerate() {
        let on = d.play_platform == p;
        let x = X0 + i as f32 * 118.0;
        if kit
            .choice(
                &format!("play-side-{i}"),
                label,
                x,
                TABS_Y,
                110.0,
                style::SMALL,
                on,
                tip,
            )
            .clicked
        {
            d.play_platform = p;
        }
    }
    // Look for a headset quietly once, so the Quest card has something to say.
    if !d.quest_auto_checked && !d.demo {
        d.quest_auto_checked = true;
        if d.quest_conn.status.is_none() && !d.quest_conn.checking {
            d.check_quest(ctx, false);
        }
    }
    if d.dialogs.take(LAUNCH_ANYWAY).is_some_and(|a| a.is_yes()) {
        start(d, ctx);
    }
    match d.play_platform {
        Platform::Pc => pc(d, kit, ctx),
        Platform::Quest => quest_hero(d, kit, ctx),
    }
}

/// The title and the status line under it.
fn hero_text(kit: &Kit, status: &str, color: egui::Color32) {
    kit.text(X0, TITLE_Y, "Echo VR", style::display(36.0), style::TEXT);
    kit.text_fit(X0, STATUS_Y, 860.0, status, style::body(15.0), color);
}

fn hint(kit: &Kit, y: f32, text: &str, color: egui::Color32) {
    if !text.is_empty() {
        kit.text_fit(X0, y, 860.0, text, style::body(13.0), color);
    }
}

/// A running job in place of a big button: its progress and a small Cancel.
fn job_row(d: &mut Dashboard, kit: &mut Kit, key: &str, id: &str, label: &str, f: Option<f32>) {
    kit.progress(X0, PLAY_Y + 8.0, COL_W - 132.0, f, label);
    if kit
        .flat_button(
            key,
            Variant::Ghost,
            None,
            "Cancel",
            X0 + COL_W - 120.0,
            PLAY_Y + (style::BIG - style::SMALL) / 2.0,
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

fn card_x(i: usize) -> (f32, f32) {
    let w = ((CW - 32.0) / 3.0).floor();
    (X0 + i as f32 * (w + 16.0), w)
}

/// A card's frame: banner title, a bold headline and a muted line. Returns the card's x
/// and width, and the y of its action row (one full-width row at the bottom).
fn info_card(
    kit: &Kit,
    slot: usize,
    title: &str,
    headline: &str,
    sub: &str,
    sub_color: egui::Color32,
) -> (f32, f32, f32) {
    let (x, w) = card_x(slot);
    kit.titled_card(x, CARDS_Y, w, CARD_H, title);
    let font = style::bold(16.0);
    kit.text_fit(
        x + 20.0,
        CARDS_Y + 50.0,
        w - 40.0,
        headline,
        font,
        style::TEXT,
    );
    kit.text_fit(
        x + 20.0,
        CARDS_Y + 74.0,
        w - 40.0,
        sub,
        style::body(12.0),
        sub_color,
    );
    (x, w, CARDS_Y + CARD_H - 18.0 - style::MID)
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

// ---- PC ----

/// The launch options in effect, for the status line.
fn options_line(d: &Dashboard) -> Vec<String> {
    let p = &d.state.profile;
    let mut parts = Vec::new();
    if p.runtime == Runtime::Flat && p.spectator {
        parts.push("Spectator".into());
    }
    if p.windowed {
        parts.push("Windowed".into());
    }
    if !p.extra_args.trim().is_empty() {
        parts.push(p.extra_args.trim().to_string());
    }
    parts
}

/// What PLAY does.
enum Main {
    Play,
    Stop,
    Install(VersionEntry),
    Reinstall(VersionEntry),
    Patch(String),
    SetUpRevive,
    Nothing,
}

/// The status line, the PLAY button's label (and second line), what it does, whether it
/// is enabled, and a hint under it.
struct Action {
    status: String,
    status_color: egui::Color32,
    label: String,
    sub: Option<String>,
    main: Main,
    enabled: bool,
    hint: String,
    hint_color: egui::Color32,
}

fn action(d: &mut Dashboard, target: &Target, installing: bool) -> Action {
    let needs_revive = d.state.profile.runtime == Runtime::Revive
        && (cfg!(windows) || d.demo)
        && d.revive_dir().is_none();
    let running = d.game().is_running();
    let ours = d.child.is_some();
    let mut a = Action {
        status: String::new(),
        status_color: style::TEXT_DIM,
        label: "PLAY".into(),
        sub: None,
        main: Main::Nothing,
        enabled: false,
        hint: String::new(),
        hint_color: style::TEXT_MUTED,
    };
    let line = |state: &str, root: &str| {
        let mut parts = vec![state.to_string()];
        parts.extend(options_line(d));
        parts.push(root.to_string());
        parts.join("  ·  ")
    };
    match target {
        Target::Installed(v) => {
            let lobby_ok = d.state.last_lobby.trim().is_empty()
                || launch::lobby_uuid(&d.state.last_lobby).is_some();
            if running && ours {
                a.status = line("Running", &v.root);
                (a.label, a.main, a.enabled) = ("STOP".into(), Main::Stop, true);
            } else if running {
                a.status = line("Running", &v.root);
                a.label = "RUNNING".into();
                a.hint = "Echo VR was started outside the launcher.".into();
            } else if d.state.owner == Some(false) && !v.patched {
                a.status = line("Needs the licence patch", &v.root);
                (a.label, a.main) = ("PATCH".into(), Main::Patch(v.id.clone()));
                a.sub = Some("Licence patch".into());
                a.enabled = !d.any_job();
                a.hint = "New players need a personal licence patch: authorize with Discord to get yours.".into();
            } else if needs_revive {
                a.status = line("SteamVR is not set up", &v.root);
                (a.label, a.main) = ("SET UP STEAMVR".into(), Main::SetUpRevive);
                a.sub = Some("Installs Revive".into());
                a.enabled = !d.any_job();
                a.hint = "Setting up SteamVR (Revive) asks for administrator rights.".into();
            } else {
                a.status = line("Ready", &v.root);
                (a.main, a.enabled) = (Main::Play, lobby_ok);
                if !lobby_ok {
                    a.hint = "Fix or clear the lobby link to play.".into();
                    a.hint_color = style::DANGER;
                }
            }
        }
        Target::Missing(v) => {
            a.status = format!("Game files missing  ·  {}", v.root);
            a.status_color = style::DANGER;
            let entry = v.catalog_id.as_ref().and_then(|cid| {
                d.catalog
                    .as_ref()
                    .and_then(|c| c.pc().find(|e| &e.id == cid).cloned())
            });
            match entry {
                Some(e) if !v.external => {
                    a.label = "REINSTALL".into();
                    a.sub = e.size.map(gb);
                    a.main = Main::Reinstall(e);
                    a.enabled = !d.any_job();
                }
                _ => {
                    a.hint =
                        "The folder is gone. Pick its new location: Version → Add existing folder."
                            .into();
                    a.hint_color = style::DANGER;
                }
            }
        }
        Target::Available(e) => {
            let root = versions_root(d, &e.id);
            let size = e.size.map(gb).unwrap_or_else(|| "size unknown".into());
            let state = if installing {
                "Installing"
            } else {
                "Not installed"
            };
            a.status = format!("{state}  ·  {size}  ·  into {root}");
            let free = d.free_bytes();
            let short = matches!((e.size, free), (Some(need), Some(free)) if need > free);
            if let (true, Some(free)) = (short, free) {
                a.hint = format!(
                    "Not enough space: needs {}, {} free. Pick another library in Settings.",
                    gb(e.size.unwrap_or_default()),
                    gb(free)
                );
                a.hint_color = style::DANGER;
            } else if d.state.versions.is_empty() {
                a.hint =
                    "Already have Echo VR? Add its folder or find your Meta install under Version."
                        .into();
            }
            a.label = "INSTALL".into();
            a.sub = e.size.map(gb);
            a.main = Main::Install(e.clone());
            a.enabled = !short && !d.any_job();
        }
        Target::None => {
            a.status = "The version list is loading. You can also add a folder you already have under Version.".into();
        }
    }
    a
}

fn pc(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    let target = d.target();
    let (id, name) = match &target {
        Target::Installed(v) | Target::Missing(v) => (Some(v.id.clone()), v.name.clone()),
        Target::Available(e) => (Some(e.id.clone()), e.name.clone()),
        Target::None => (None, "No versions yet".into()),
    };
    let job_id = id.filter(|id| d.jobs.contains_key(id)).or_else(|| {
        d.jobs
            .contains_key(setup::REVIVE_JOB)
            .then(|| setup::REVIVE_JOB.into())
    });
    let job = job_id
        .as_ref()
        .and_then(|id| d.jobs.get(id))
        .map(|j| (j.label.clone(), j.fraction));
    let a = action(d, &target, job.is_some());
    hero_text(kit, &a.status, a.status_color);

    // Version and headset.
    let hx = X0 + VERSION_W + 12.0;
    kit.caps(X0, LABEL_Y, "Version", style::TEXT_MUTED);
    kit.caps(hx, LABEL_Y, "Headset", style::TEXT_MUTED);
    let anchor = kit.dropdown_face(
        VERSION_KEY,
        &name,
        X0,
        PICK_Y,
        VERSION_W,
        style::MID,
        true,
        "Choose a version: installed ones, or ones you can install",
    );
    version_menu(d, kit, ctx, anchor);
    let modes: Vec<String> = Runtime::ALL.iter().map(|r| r.label().to_string()).collect();
    let msel = Runtime::ALL
        .iter()
        .position(|r| *r == d.state.profile.runtime)
        .unwrap_or(0);
    if let Some(i) = kit.dropdown(
        "mode",
        &modes,
        msel,
        hx,
        PICK_Y,
        COL_W - VERSION_W - 12.0,
        style::MID,
        "How Echo VR reaches your headset",
    ) {
        d.state.profile.runtime = Runtime::ALL[i];
        d.save();
    }

    // PLAY, or the running job in its place.
    if let (Some((label, fraction)), Some(id)) = (&job, &job_id) {
        job_row(d, kit, "job-cancel", id, label, *fraction);
    } else {
        let tip = match &a.main {
            Main::Play => "Start Echo VR",
            Main::Stop => "Close Echo VR",
            Main::Install(_) => "Download and install this version into your library",
            Main::Reinstall(_) => "Download this version again into its folder",
            Main::Patch(_) => "Opens Discord in your browser to get your personal patch",
            Main::SetUpRevive => "Download and install Revive, which runs Echo VR on SteamVR",
            Main::Nothing => "",
        };
        let sub = a.sub.as_deref();
        if kit
            .play_button(
                "play-main",
                &a.label,
                sub,
                X0,
                PLAY_Y,
                COL_W,
                a.enabled,
                tip,
            )
            .clicked
        {
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
                Main::Nothing => {}
            }
        }
    }
    hint(kit, HINT_Y, &a.hint, a.hint_color);

    // Launch options, right here when ticked.
    let mut show = d.state.show_launch_options;
    if kit.toggle(
        "launch-options",
        &mut show,
        "Launch options",
        X0,
        OPTS_Y,
        true,
        "Show the options Echo VR starts with",
    ) {
        d.state.show_launch_options = show;
        d.save();
    }
    if show {
        launch_options(d, kit);
    }

    lobby_card(d, kit, ctx);
    match &target {
        Target::Installed(v) => updates_card(d, kit, ctx, Some(v)),
        _ => updates_card(d, kit, ctx, None),
    }
    quest_card(d, kit, ctx, 2);
}

fn versions_root(d: &Dashboard, id: &str) -> String {
    crate::core::launcher::versions::root_for(&d.state.library, id)
}

/// The version picker's menu: installed versions, versions to install, and actions.
fn version_menu(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context, anchor: Rect) {
    enum Pick {
        Select(String),
        AddFolder,
        FindMeta,
        Manage,
    }
    let current = match d.target() {
        Target::Installed(v) | Target::Missing(v) => Some(v.id),
        Target::Available(e) => Some(e.id),
        Target::None => None,
    };
    let mut items = Vec::new();
    let mut picks = Vec::new();
    let mut push = |item: MenuItem, pick: Option<Pick>| {
        items.push(item);
        picks.push(pick);
    };
    if !d.state.versions.is_empty() {
        push(MenuItem::Header("INSTALLED".into()), None);
    }
    for v in &d.state.versions {
        let ok = d.demo || paths::has_echo_install(&v.root);
        let mut row = MenuItem::row(&v.name).sub(if ok {
            v.root.clone()
        } else {
            format!("Files missing — {}", v.root)
        });
        if v.external {
            row = row.badge("Existing", style::CHIP_OFF);
        }
        push(
            row.current(current.as_deref() == Some(v.id.as_str())),
            Some(Pick::Select(v.id.clone())),
        );
    }
    let available: Vec<VersionEntry> = d
        .catalog
        .as_ref()
        .map(|c| d.state.not_installed(c).into_iter().cloned().collect())
        .unwrap_or_default();
    if !available.is_empty() {
        push(MenuItem::Header("NOT INSTALLED".into()), None);
    }
    for e in &available {
        let mut sub = e.size.map(gb).unwrap_or_default();
        if !e.notes.is_empty() {
            if !sub.is_empty() {
                sub.push_str("  ·  ");
            }
            sub.push_str(&e.notes);
        }
        let mut row = MenuItem::row(&e.name)
            .icon(Icon::Download)
            .sub(sub)
            .tip(format!("Installs into {}", versions_root(d, &e.id)));
        if !e.channel.is_empty() {
            let color = if e.channel == "stable" {
                style::OK
            } else {
                style::CHIP_OFF
            };
            row = row.badge(e.channel.clone(), color);
        }
        push(
            row.current(current.as_deref() == Some(e.id.as_str())),
            Some(Pick::Select(e.id.clone())),
        );
    }
    push(MenuItem::Divider, None);
    push(
        MenuItem::row("Add existing folder…")
            .icon(Icon::Folder)
            .item(),
        Some(Pick::AddFolder),
    );
    if cfg!(windows) || d.demo {
        push(
            MenuItem::row("Find Meta install")
                .icon(Icon::Monitor)
                .tip("Add Echo VR from your Meta (Oculus) library")
                .item(),
            Some(Pick::FindMeta),
        );
    }
    push(
        MenuItem::row("Manage versions…").icon(Icon::Gear).item(),
        Some(Pick::Manage),
    );
    let w = anchor.width().max(420.0);
    let Some(i) = kit.menu_popup(VERSION_KEY, anchor, w, &items) else {
        return;
    };
    match picks.into_iter().nth(i).flatten() {
        Some(Pick::Select(id)) => {
            d.state.selected = Some(id);
            d.save();
        }
        Some(Pick::AddFolder) => versions::add_existing(d),
        Some(Pick::FindMeta) => versions::find_meta(d),
        Some(Pick::Manage) => d.page = Page::Versions,
        None => {}
    }
    ctx.request_repaint();
}

/// The launch options, in a box under their checkbox.
fn launch_options(d: &mut Dashboard, kit: &mut Kit) {
    kit.card(X0, OPTS_CARD_Y, COL_W, OPTS_CARD_H);
    let (x, y) = (X0 + 20.0, OPTS_CARD_Y + 14.0);
    let flat = d.state.profile.runtime == Runtime::Flat;
    let mut changed = kit.toggle(
        "opt-windowed",
        &mut d.state.profile.windowed,
        "Windowed",
        x,
        y,
        true,
        "Run in a window (-windowed)",
    );
    changed |= kit.toggle(
        "opt-spectator",
        &mut d.state.profile.spectator,
        "Spectator stream",
        x + 200.0,
        y,
        flat,
        "Flat mode only: join as a spectator (-spectatorstream)",
    );
    kit.caps(x, y + 50.0, "Extra arguments", style::TEXT_MUTED);
    changed |= kit.input(
        "opt-args",
        &mut d.state.profile.extra_args,
        x + 170.0,
        y + 40.0,
        COL_W - 40.0 - 170.0,
        32.0,
        "e.g. -mp -http",
        false,
        "Additional arguments for echovr.exe",
    );
    if changed {
        d.save();
    }
}

fn lobby_card(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    let lobby = d.state.last_lobby.trim().to_string();
    let invalid = !lobby.is_empty() && launch::lobby_uuid(&lobby).is_none();
    let running = d.game().is_running();
    let (headline, sub, color) = if invalid {
        (
            "Join a lobby",
            "That doesn't look like a lobby link.",
            style::DANGER,
        )
    } else if running {
        (
            "Join a lobby",
            "Echo VR is running -- join from the game for now.",
            style::TEXT_MUTED,
        )
    } else if d.clip_lobby.is_some() {
        (
            "Lobby link on your clipboard",
            "Paste it to start straight into that lobby.",
            style::TEXT_MUTED,
        )
    } else if !lobby.is_empty() {
        (
            "Lobby ready",
            "PLAY and Join start straight into this lobby.",
            style::TEXT_MUTED,
        )
    } else {
        (
            "Join a lobby",
            "Paste a spark:// link or lobby ID to start straight into it.",
            style::TEXT_MUTED,
        )
    };
    let (x, w, ay) = info_card(kit, 0, "Join lobby", headline, sub, color);
    if kit
        .flat_button(
            "lobby-paste",
            Variant::Secondary,
            None,
            "Paste",
            x + w - 20.0 - 90.0,
            CARDS_Y + 10.0,
            90.0,
            style::SMALL,
            true,
            "Paste a lobby link from your clipboard",
        )
        .clicked
    {
        let clip = d.clip_lobby.take().or_else(|| {
            arboard::Clipboard::new()
                .ok()
                .and_then(|mut c| c.get_text().ok())
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty() && t.len() < 300)
        });
        if let Some(clip) = clip {
            d.state.last_lobby = clip;
            d.save();
        }
    }
    let bw = 110.0;
    if kit.input(
        "lobby",
        &mut d.state.last_lobby,
        x + 20.0,
        ay,
        w - 40.0 - bw - 10.0,
        style::MID,
        "spark:// link or lobby ID",
        invalid,
        "Starts Echo VR straight into this lobby (-lobbyid)",
    ) {
        d.save();
    }
    let installed = matches!(d.target(), Target::Installed(_));
    let can = !invalid && !lobby.is_empty() && !running && installed;
    if kit
        .flat_button(
            "join",
            Variant::Secondary,
            None,
            "Join",
            x + w - 20.0 - bw,
            ay,
            bw,
            style::MID,
            can,
            "Start Echo VR and join this lobby",
        )
        .clicked
    {
        try_start(d, ctx);
    }
}

fn updates_card(
    d: &mut Dashboard,
    kit: &mut Kit,
    ctx: &egui::Context,
    v: Option<&InstalledVersion>,
) {
    let (headline, sub) = match v {
        Some(v) => (
            d.update_note
                .get(&v.id)
                .cloned()
                .unwrap_or_else(|| "Community patches".into()),
            "Keeps the game files of this version current.",
        ),
        None => (
            "Nothing to update yet".into(),
            "Install the version first; updates run from here.",
        ),
    };
    let (x, w, ay) = info_card(kit, 1, "Updates", &headline, sub, style::TEXT_MUTED);
    if let Some(j) = v.and_then(|v| d.jobs.get(&v.id)) {
        let (label, fraction) = (j.label.clone(), j.fraction);
        kit.progress(x + 20.0, ay + 2.0, w - 40.0 - 110.0, fraction, &label);
        if kit
            .flat_button(
                "upd-cancel",
                Variant::Ghost,
                None,
                "Cancel",
                x + w - 20.0 - 100.0,
                ay + (style::MID - style::SMALL) / 2.0,
                100.0,
                style::SMALL,
                true,
                "Stop",
            )
            .clicked
        {
            if let Some(v) = v {
                d.cancel_job(&v.id);
            }
        }
        return;
    }
    let ok =
        v.is_some_and(|v| d.demo || paths::has_echo_install(&v.root)) && !d.game().is_running();
    if kit
        .flat_button(
            "upd-check",
            Variant::Secondary,
            Some(Icon::Refresh),
            "Check for updates",
            x + 20.0,
            ay,
            w - 40.0,
            style::MID,
            ok,
            "Download any changed game files",
        )
        .clicked
    {
        if let Some(v) = v {
            versions::update(d, ctx, v.clone());
        }
    }
}

fn quest_card(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context, slot: usize) {
    let ready = d.quest_conn.status == Some(Status::Ready);
    let (title, sub) = quest_texts(d);
    let (x, w, ay) = info_card(kit, slot, "Quest", &title, &sub, style::TEXT_MUTED);
    let installed = ready && d.quest_info.as_ref().is_some_and(|i| i.installed);
    if installed {
        let half = ((w - 52.0) / 2.0).floor();
        if kit
            .flat_button(
                "qc-launch",
                Variant::Secondary,
                Some(Icon::Play),
                "Play on Quest",
                x + 20.0,
                ay,
                half,
                style::MID,
                !d.quest_busy,
                "Start Echo VR on the headset",
            )
            .clicked
        {
            quest_launch(d, ctx);
        }
        if kit
            .flat_button(
                "qc-stop",
                Variant::Secondary,
                Some(Icon::Stop),
                "Stop",
                x + 32.0 + half,
                ay,
                half,
                style::MID,
                !d.quest_busy,
                "Close Echo VR on the headset",
            )
            .clicked
        {
            quest_stop(d, ctx);
        }
    } else {
        let busy = d.quest_conn.checking || d.quest_busy;
        if kit
            .flat_button(
                "qc-connect",
                Variant::Secondary,
                Some(Icon::Headset),
                "Connect Quest",
                x + 20.0,
                ay,
                w - 40.0,
                style::MID,
                !busy,
                "Check the USB connection to your Quest",
            )
            .clicked
        {
            d.check_quest(ctx, true);
        }
    }
}

fn quest_texts(d: &Dashboard) -> (String, String) {
    match (d.quest_conn.checking, d.quest_conn.status) {
        (true, _) => (
            "Checking...".into(),
            "Looking for your headset over USB".into(),
        ),
        (_, Some(Status::Ready)) => match &d.quest_info {
            Some(i) => (
                i.version_label(),
                i.device.clone().unwrap_or_else(|| "Quest connected".into()),
            ),
            None => (
                "Quest connected".into(),
                "Reading the installed version...".into(),
            ),
        },
        (_, Some(Status::Unauthorized)) => (
            "Allow this PC".into(),
            "Accept the USB debugging prompt in the headset".into(),
        ),
        (_, Some(Status::Ambiguous)) => (
            "Several devices".into(),
            "Pick your Quest when you connect".into(),
        ),
        (_, Some(Status::None)) => (
            "No Quest connected".into(),
            "Plug it in by USB with developer mode on".into(),
        ),
        (_, None) => (
            "Not checked yet".into(),
            "Plug in your Quest by USB to launch Echo VR on it".into(),
        ),
    }
}

fn quest_launch(d: &mut Dashboard, ctx: &egui::Context) {
    d.quest_busy = true;
    d.worker.spawn(ctx, |tx| {
        tx.send(Msg::QuestAction(quest::launch().map_err(|e| {
            UiError::from_anyhow(&e, "Couldn't start Echo VR")
        })))
    });
}

fn quest_stop(d: &mut Dashboard, ctx: &egui::Context) {
    d.quest_busy = true;
    d.worker.spawn(ctx, |tx| {
        tx.send(Msg::QuestAction(
            quest::stop().map_err(|e| UiError::from_anyhow(&e, "Couldn't stop Echo VR")),
        ))
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
    let lobby = launch::lobby_uuid(&d.state.last_lobby);
    let revive_dir = if d.state.profile.runtime == Runtime::Revive {
        revive::find_revive_dir()
    } else {
        None
    };
    let result = launch::build(
        &d.state.profile,
        &exe,
        revive_dir.as_deref(),
        lobby.as_deref(),
    )
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

// ---- Quest ----

/// The Quest side's status line, and what to show as the device.
fn quest_status(d: &Dashboard) -> (String, String) {
    let device = d.quest_info.as_ref().and_then(|i| i.device.clone());
    match (d.quest_conn.checking, d.quest_conn.status) {
        (true, _) => (
            "Looking for your headset over USB...".into(),
            "Checking...".into(),
        ),
        (_, Some(Status::Ready)) => {
            let status = match &d.quest_info {
                Some(i) if i.installed => format!("Installed  ·  {}", i.version_label()),
                Some(_) => "Not installed on this Quest".into(),
                None => "Reading the installed version...".into(),
            };
            (status, device.unwrap_or_else(|| "Quest connected".into()))
        }
        (_, Some(Status::Unauthorized)) => (
            "Accept the USB debugging prompt in the headset to allow this PC.".into(),
            "Waiting for permission".into(),
        ),
        (_, Some(Status::Ambiguous)) => (
            "Several devices are connected: pick your Quest when you connect.".into(),
            "Several devices".into(),
        ),
        (_, Some(Status::None)) => (
            "No Quest found. Plug it in by USB with developer mode on.".into(),
            "Not connected".into(),
        ),
        (_, None) => (
            "Plug in your Quest by USB to install or launch Echo VR on it.".into(),
            "Not checked yet".into(),
        ),
    }
}

fn quest_hero(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    let ready = d.quest_conn.status == Some(Status::Ready);
    let installed = ready && d.quest_info.as_ref().is_some_and(|i| i.installed);
    let known = ready && d.quest_info.is_some();
    let busy = d.quest_conn.checking || d.quest_busy || d.any_job();
    let (status, device) = quest_status(d);
    hero_text(kit, &status, style::TEXT_DIM);

    kit.caps(X0, LABEL_Y, "Device", style::TEXT_MUTED);
    let font = style::bold(16.0);
    kit.text_fit(X0, PICK_Y + 9.0, COL_W - 170.0, &device, font, style::TEXT);
    if ready
        && kit
            .flat_button(
                "q-recheck",
                Variant::Ghost,
                Some(Icon::Refresh),
                "Check again",
                X0 + COL_W - 150.0,
                PICK_Y + (style::MID - style::SMALL) / 2.0,
                150.0,
                style::SMALL,
                !busy,
                "Read the headset's state again",
            )
            .clicked
    {
        d.check_quest(ctx, true);
    }

    let job = d
        .jobs
        .get(setup::QUEST_JOB)
        .map(|j| (j.label.clone(), j.fraction));
    let half = ((COL_W - 12.0) / 2.0).floor();
    let mut hint_text = "";
    let mut second_row = false;
    if let Some((label, fraction)) = job {
        job_row(d, kit, "q-cancel", setup::QUEST_JOB, &label, fraction);
        hint_text = "Keep the headset connected until this finishes.";
    } else if installed {
        if kit
            .play_button(
                "q-launch",
                "PLAY ON QUEST",
                None,
                X0,
                PLAY_Y,
                COL_W,
                !d.quest_busy,
                "Start Echo VR on the headset",
            )
            .clicked
        {
            quest_launch(d, ctx);
        }
        second_row = true;
        if kit
            .flat_button(
                "q-stop",
                Variant::Secondary,
                Some(Icon::Stop),
                "Stop",
                X0,
                SECOND_Y,
                half,
                style::MID,
                !d.quest_busy,
                "Close Echo VR on the headset",
            )
            .clicked
        {
            quest_stop(d, ctx);
        }
        if kit
            .flat_button(
                "q-update",
                Variant::Secondary,
                Some(Icon::Refresh),
                "Update",
                X0 + half + 12.0,
                SECOND_Y,
                half,
                style::MID,
                !busy,
                "Copy the latest game files to your Quest",
            )
            .clicked
        {
            setup::quest_update(d, ctx);
        }
    } else if known {
        if kit
            .play_button(
                "q-install",
                "INSTALL ON QUEST",
                None,
                X0,
                PLAY_Y,
                COL_W,
                !busy,
                "Download Echo VR and install it on your Quest",
            )
            .clicked
        {
            setup::ask_quest_install(d);
        }
        if d.state.owner == Some(false) {
            second_row = true;
            if kit
                .flat_button(
                    "q-link",
                    Variant::Secondary,
                    None,
                    "APK from a link…",
                    X0,
                    SECOND_Y,
                    half,
                    style::MID,
                    !busy,
                    "Install a patched APK from a link you already have",
                )
                .clicked
            {
                d.overlay = Some(setup::Overlay::PatchLink {
                    target: setup::LinkFor::Quest,
                    url: String::new(),
                });
            }
            hint_text = "New players get a personal patched APK through Discord.";
        }
    } else if kit
        .play_button(
            "q-connect",
            "CONNECT",
            None,
            X0,
            PLAY_Y,
            COL_W,
            !d.quest_conn.checking && !d.quest_busy,
            "Look for your Quest over USB",
        )
        .clicked
    {
        d.check_quest(ctx, true);
    }
    let hy = if second_row {
        SECOND_Y + style::MID + 12.0
    } else {
        HINT_Y
    };
    hint(kit, hy, hint_text, style::TEXT_MUTED);

    // Cards: install, update, help.
    let (x, w, ay) = info_card(
        kit,
        0,
        "Install",
        if installed {
            "Reinstall"
        } else {
            "Fresh install"
        },
        "APK and game data, patched if you need it.",
        style::TEXT_MUTED,
    );
    if kit
        .flat_button(
            "qc-install",
            Variant::Secondary,
            Some(Icon::Download),
            if installed {
                "Reinstall"
            } else {
                "Install on Quest"
            },
            x + 20.0,
            ay,
            w - 40.0,
            style::MID,
            !busy,
            "Download Echo VR and install it on your Quest over USB",
        )
        .clicked
    {
        setup::ask_quest_install(d);
    }
    let (x, w, ay) = info_card(
        kit,
        1,
        "Update",
        "Community patches",
        "Copies changed files to the headset.",
        style::TEXT_MUTED,
    );
    if kit
        .flat_button(
            "qc-update",
            Variant::Secondary,
            Some(Icon::Refresh),
            "Update on Quest",
            x + 20.0,
            ay,
            w - 40.0,
            style::MID,
            !busy && (installed || !ready),
            "Copy the latest game files to your Quest",
        )
        .clicked
    {
        setup::quest_update(d, ctx);
    }
    let (x, w, ay) = info_card(
        kit,
        2,
        "Help",
        "Developer mode",
        "Needed for USB installs and launching.",
        style::TEXT_MUTED,
    );
    if kit
        .flat_button(
            "qc-help",
            Variant::Ghost,
            Some(Icon::Info),
            "How to enable it",
            x + 20.0,
            ay,
            w - 40.0,
            style::MID,
            true,
            "Opens a guide in your browser",
        )
        .clicked
    {
        crate::core::platform::open_url(
            "https://learn.adafruit.com/sideloading-on-oculus-quest/enable-developer-mode",
        );
    }
}
