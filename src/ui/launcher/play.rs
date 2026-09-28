//! Play page: the selected version, a big PLAY button, version and launch-mode
//! dropdowns, launch options and a lobby link, like one of the wizards' steps. The Quest
//! variant starts Echo on the headset.

use egui::{pos2, vec2, Order, Rect, Sense};

use super::{header, para, versions, Dashboard, Msg, CX, HEADER_Y};
use crate::core::adb::devices::Status;
use crate::core::error::UiError;
use crate::core::launcher::catalog::Platform;
use crate::core::launcher::store::{InstalledVersion, Runtime};
use crate::core::launcher::{launch, quest};
use crate::core::{paths, revive};
use crate::ui::dialogs::Icon as DlgIcon;
use crate::ui::frame::{self, Chip};
use crate::ui::kit::{Btn, Kit};
use crate::ui::launcher::Open;
use crate::ui::style;
use crate::ui::theme;

const LAUNCH_ANYWAY: &str = "launch-anyway";
const DEV_MODE_URL: &str =
    "https://learn.adafruit.com/sideloading-on-oculus-quest/enable-developer-mode";
/// Rows under the header: the text lines, the big button, the middle-button row, the
/// lobby row and the two info rows.
const TEXT_Y: f32 = 132.0;
const BIG_Y: f32 = 176.0;
const ROW_Y: f32 = 242.0;
const LOBBY_Y: f32 = 304.0;
const INFO_Y: f32 = 372.0;
const INFO_W: f32 = 420.0;
const INFO_X: f32 = CX - (INFO_W + 10.0 + 141.0) / 2.0;

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    platform_chips(d, kit, ctx);
    if d.dialogs.take(LAUNCH_ANYWAY).is_some_and(|a| a.is_yes()) {
        start(d);
    }
    match d.play_platform {
        Platform::Pc if d.state.versions.is_empty() => welcome(d, kit, ctx),
        Platform::Pc => pc(d, kit, ctx),
        Platform::Quest => quest_page(d, kit, ctx),
    }
}

/// PC | Quest as step chips in the bottom bar.
fn platform_chips(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    let labels = ["PC", "Quest"];
    let states = if d.play_platform == Platform::Pc {
        [Chip::Current, Chip::Upcoming]
    } else {
        [Chip::Upcoming, Chip::Current]
    };
    let (_, total) = frame::chip_widths(kit, &labels);
    let x = frame::CONTENT_X + ((frame::CONTENT_W - total) / 2.0).floor();
    let tips = ["Play Echo VR on this PC", "Play Echo VR on your Quest"];
    if let Some(i) = frame::chips(kit, "play-platform", x, &labels, &states, false, &tips) {
        d.play_platform = if i == 0 {
            Platform::Pc
        } else {
            Platform::Quest
        };
        if d.play_platform == Platform::Quest
            && d.quest_conn.status.is_none()
            && !d.quest_conn.checking
        {
            d.check_quest(ctx, false);
        }
    }
}

/// A big button centered in the section box.
fn big(kit: &mut Kit, key: &str, text: &str, enabled: bool, tip: &str) -> bool {
    kit.button(
        key,
        Btn::Big,
        text,
        18.0,
        CX - Btn::Big.w() / 2.0,
        BIG_Y,
        enabled,
        tip,
    )
}

/// An info row: a `SpecialLabel` with the text and a small button next to it.
fn info_row(
    kit: &mut Kit,
    key: &str,
    y: f32,
    text: &str,
    button: &str,
    enabled: bool,
    tip: &str,
) -> bool {
    kit.special_label(
        INFO_X,
        y,
        INFO_W,
        25.0,
        text,
        11.0,
        theme::LABEL_BG,
        theme::WHITE,
    );
    kit.button(
        key,
        Btn::Small,
        button,
        11.0,
        INFO_X + INFO_W + 10.0,
        y,
        enabled,
        tip,
    )
}

// ---- PC ----

fn welcome(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    header(kit, "Welcome to Echo VR", HEADER_Y);
    para(
        kit,
        "Install the game, or add an install that is already on this PC.",
        TEXT_Y + 6.0,
        14.0,
        style::TEXT,
    );
    let entry = d.catalog.as_ref().and_then(|c| c.pc().next().cloned());
    let job = entry
        .as_ref()
        .and_then(|e| d.jobs.get(&e.id))
        .map(|j| (j.label.clone(), j.fraction));
    if let (Some((label, fraction)), Some(e)) = (job, entry.as_ref()) {
        kit.progress(CX - 220.0, BIG_Y + 12.0, 440.0, 26.0, fraction, &label);
        if kit.button(
            "welcome-cancel",
            Btn::Middle,
            "Cancel",
            14.0,
            CX - Btn::Middle.w() / 2.0,
            ROW_Y,
            true,
            "Stop the download",
        ) {
            d.cancel_job(&e.id);
        }
    } else {
        let tip = "Download and install the current Echo VR build into your library";
        if big(
            kit,
            "welcome-install",
            "Install Echo VR",
            entry.is_some(),
            tip,
        ) {
            if let Some(e) = entry {
                versions::install(d, ctx, e);
            }
        }
        if kit.button(
            "welcome-add",
            Btn::Middle,
            "Add existing folder",
            13.0,
            CX - Btn::Middle.w() / 2.0,
            ROW_Y,
            true,
            "Use an Echo VR install that is already on this PC",
        ) {
            versions::add_existing(d);
        }
    }
    para(
        kit,
        "Need the licence patch or the Revive setup? Use \"Install PC\" in the sidebar.",
        LOBBY_Y + 4.0,
        12.0,
        style::TEXT_DIM,
    );
    quest_row(d, kit, ctx, INFO_Y);
    None
}

fn subline(d: &Dashboard) -> String {
    let p = &d.state.profile;
    let mut parts = vec![p.runtime.label().to_string()];
    if p.runtime == Runtime::Flat && p.spectator {
        parts.push("Spectator".into());
    }
    if p.windowed {
        parts.push("Windowed".into());
    }
    if !p.extra_args.trim().is_empty() {
        parts.push(p.extra_args.trim().to_string());
    }
    parts.join("  ·  ")
}

fn pc(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    let v = d.state.selected_version().cloned()?;
    let present = d.demo || paths::has_echo_install(&v.root);
    header(kit, &v.name, HEADER_Y);
    para(kit, &subline(d), TEXT_Y, 14.0, style::TEXT);
    let (path, color) = if present {
        (v.root.clone(), style::TEXT_DIM)
    } else {
        (format!("{}  --  files missing", v.root), theme::MARK_BAD)
    };
    kit.text_fit_center(
        super::IX,
        TEXT_Y + 20.0,
        super::IW,
        16.0,
        &path,
        theme::arial(12.0),
        color,
    );

    // PLAY / STOP.
    if d.game().is_running() {
        let ours = d.child.is_some();
        let tip = if ours {
            "Close Echo VR"
        } else {
            "Echo VR was started outside the launcher"
        };
        if big(
            kit,
            "stop",
            if ours { "Stop" } else { "Running" },
            ours,
            tip,
        ) {
            if let Some(mut c) = d.child.take() {
                let _ = c.kill();
            }
        }
    } else {
        let lobby_ok = d.state.last_lobby.trim().is_empty()
            || launch::lobby_uuid(&d.state.last_lobby).is_some();
        if big(kit, "play", "PLAY", present && lobby_ok, "Start Echo VR") {
            try_start(d);
        }
    }

    // Version, launch mode, options.
    let (vw, mw, ow, gap) = (250.0, 212.0, 150.0, 12.0);
    let x0 = CX - (vw + mw + ow + 2.0 * gap) / 2.0;
    let names: Vec<String> = d.state.versions.iter().map(|v| v.name.clone()).collect();
    let sel = d
        .state
        .versions
        .iter()
        .position(|x| x.id == v.id)
        .unwrap_or(0);
    if let Some(i) = kit.dropdown(
        "version",
        Btn::Middle,
        &names,
        sel,
        x0,
        ROW_Y,
        vw,
        "Choose the version to play",
    ) {
        d.state.selected = Some(d.state.versions[i].id.clone());
        d.save();
    }
    let modes: Vec<String> = Runtime::ALL.iter().map(|r| r.label().to_string()).collect();
    let msel = Runtime::ALL
        .iter()
        .position(|r| *r == d.state.profile.runtime)
        .unwrap_or(0);
    if let Some(i) = kit.dropdown(
        "mode",
        Btn::Middle,
        &modes,
        msel,
        x0 + vw + gap,
        ROW_Y,
        mw,
        "How Echo VR reaches your headset",
    ) {
        d.state.profile.runtime = Runtime::ALL[i];
        d.save();
    }
    let ox = x0 + vw + mw + 2.0 * gap;
    if kit.button_w(
        "options",
        Btn::Middle,
        "Options",
        13.0,
        ox,
        ROW_Y,
        ow,
        true,
        "Windowed, spectator stream and extra arguments",
    ) {
        d.options_open = !d.options_open;
    } else if d.options_open {
        let anchor = kit.rect(ox, ROW_Y, ow, 38.0);
        options_popover(d, kit, anchor);
    }

    lobby_row(d, kit);
    update_row(d, kit, ctx, &v);
    quest_row(d, kit, ctx, INFO_Y + 36.0);
    None
}

/// Launch options in a wine panel under the Options button.
fn options_popover(d: &mut Dashboard, kit: &mut Kit, anchor: Rect) {
    let (w, h) = (300.0, 136.0);
    let pos = pos2(anchor.max.x - w, anchor.max.y + 4.0);
    let popup = Rect::from_min_size(pos, vec2(w, h));
    let ctx = kit.ctx();
    let assets = kit.assets;
    let mut changed = false;
    let mut tip = None;
    egui::Area::new(egui::Id::new("launch-options"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(&ctx, |ui| {
            ui.allocate_exact_size(vec2(w, h), Sense::hover());
            let mut k = Kit::new(ui, assets, "launch-options", false);
            k.origin = pos;
            k.section_box(0.0, 0.0, w, h, 15.0, theme::rgba(100, 0, 50, 245));
            let flat = d.state.profile.runtime == Runtime::Flat;
            changed |= k.checkbox(
                "opt-windowed",
                &mut d.state.profile.windowed,
                "Windowed",
                12.0,
                14.0,
                12.0,
                w - 28.0,
                20.0,
                false,
                true,
                "Run in a window (-windowed)",
            );
            changed |= k.checkbox(
                "opt-spectator",
                &mut d.state.profile.spectator,
                "Spectator stream",
                12.0,
                14.0,
                38.0,
                w - 28.0,
                20.0,
                false,
                flat,
                "Flat mode only: join as a spectator (-spectatorstream)",
            );
            k.text_left(
                16.0,
                66.0,
                20.0,
                "Extra arguments",
                theme::conthrax(11.0),
                theme::WHITE,
            );
            changed |= k
                .text_field(
                    "opt-args",
                    &mut d.state.profile.extra_args,
                    14.0,
                    92.0,
                    w - 28.0,
                    26.0,
                    12.0,
                    "e.g. -mp -http",
                    theme::FIELD_BG,
                    "Additional arguments for echovr.exe",
                )
                .committed;
            tip = k.tip.take();
        });
    if tip.is_some() {
        kit.tip = tip;
    }
    if changed {
        d.save();
    }
    let outside = ctx.input(|i| {
        i.pointer.any_click()
            && i.pointer
                .interact_pos()
                .is_some_and(|p| !popup.contains(p) && !anchor.contains(p))
    });
    if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        d.options_open = false;
        d.save();
    }
}

/// The lobby link field with the installer's ✓/✗ mark, and Join.
fn lobby_row(d: &mut Dashboard, kit: &mut Kit) {
    let (fw, fh) = (360.0, 26.0);
    let x0 = CX - (fw + 34.0 + Btn::Small.w()) / 2.0;
    let t = d.state.last_lobby.trim().to_string();
    let valid = !t.is_empty() && launch::lobby_uuid(&t).is_some();
    let bg = if t.is_empty() {
        theme::FIELD_BG
    } else if valid {
        theme::FIELD_BG_VALID
    } else {
        theme::FIELD_BG_INVALID
    };
    if kit
        .text_field(
            "lobby",
            &mut d.state.last_lobby,
            x0,
            LOBBY_Y,
            fw,
            fh,
            12.0,
            "spark:// link or lobby ID",
            bg,
            "Starts Echo VR straight into this lobby (-lobbyid)",
        )
        .committed
    {
        d.save();
    }
    if !t.is_empty() {
        kit.mark(
            valid,
            if valid {
                theme::MARK_OK
            } else {
                theme::MARK_BAD
            },
            22.0,
            x0 + fw + 5.0,
            LOBBY_Y + 2.0,
        );
        if kit
            .hand_area(
                "lobby-clear",
                x0 + fw + 4.0,
                LOBBY_Y,
                24.0,
                fh,
                "Click the check / cross icon to clear this field",
            )
            .clicked
        {
            d.state.last_lobby.clear();
            d.save();
        }
    }
    let running = d.game().is_running();
    if kit.button(
        "join",
        Btn::Small,
        "Join",
        11.0,
        x0 + fw + 34.0,
        LOBBY_Y,
        valid && !running,
        "Start Echo VR and join this lobby",
    ) {
        try_start(d);
    }
    let note = if !t.is_empty() && !valid {
        "That doesn't look like a lobby link."
    } else if running {
        "Echo VR is running -- join from the game for now."
    } else {
        "Paste a lobby link to start straight into it. The server browser comes with Servers."
    };
    kit.text_fit_center(
        super::IX,
        LOBBY_Y + 32.0,
        super::IW,
        16.0,
        note,
        theme::arial(12.0),
        if !t.is_empty() && !valid {
            theme::MARK_BAD
        } else {
            style::TEXT_DIM
        },
    );
}

fn update_row(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context, v: &InstalledVersion) {
    if let Some(j) = d.jobs.get(&v.id) {
        let (label, fraction) = (j.label.clone(), j.fraction);
        kit.progress(INFO_X, INFO_Y, INFO_W, 25.0, fraction, &label);
        if kit.button(
            "upd-cancel",
            Btn::Small,
            "Cancel",
            11.0,
            INFO_X + INFO_W + 10.0,
            INFO_Y,
            true,
            "Stop",
        ) {
            d.cancel_job(&v.id);
        }
        return;
    }
    let note = d
        .update_note
        .get(&v.id)
        .cloned()
        .unwrap_or_else(|| "not checked yet".into());
    let ok = (d.demo || paths::has_echo_install(&v.root)) && !d.game().is_running();
    if info_row(
        kit,
        "upd-check",
        INFO_Y,
        &format!("Updates: {note}"),
        "Check",
        ok,
        "Download any changed game files of this version",
    ) {
        versions::update(d, ctx, v.clone());
    }
}

/// The Quest connection as an info row (Connect / Play on Quest).
fn quest_row(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context, y: f32) {
    let (title, _) = quest_texts(d);
    let ready = d.quest_conn.status == Some(Status::Ready);
    let installed = ready && d.quest_info.as_ref().is_some_and(|i| i.installed);
    let busy = d.quest_conn.checking || d.quest_busy;
    let text = format!("Quest: {title}");
    if installed {
        if info_row(
            kit,
            "qr-launch",
            y,
            &text,
            "Play on Quest",
            !busy,
            "Start Echo VR on the headset",
        ) {
            quest_launch(d, ctx);
        }
    } else if info_row(
        kit,
        "qr-connect",
        y,
        &text,
        "Connect",
        !busy,
        "Check the USB connection to your Quest",
    ) {
        d.check_quest(ctx, true);
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
            "No Quest found".into(),
            "Connect by USB with developer mode on".into(),
        ),
        (_, None) => ("Not checked".into(), "Connect your Quest by USB".into()),
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

fn try_start(d: &mut Dashboard) {
    match launch::preflight(&d.state.profile) {
        Some(w) => d.dialogs.confirm(
            LAUNCH_ANYWAY,
            "Launch Echo VR",
            &format!("{w}\n\nLaunch anyway?"),
            DlgIcon::Warning,
        ),
        None => start(d),
    }
}

fn start(d: &mut Dashboard) {
    let Some(v) = d.state.selected_version().cloned() else {
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
        Ok(child) => d.child = Some(child),
        Err(e) => d.dialogs.error(
            "Couldn't start Echo VR",
            &format!("{e:#}"),
            Default::default(),
        ),
    }
}

// ---- Quest ----

fn quest_page(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    let ready = d.quest_conn.status == Some(Status::Ready);
    let installed = ready && d.quest_info.as_ref().is_some_and(|i| i.installed);
    let (title, sub) = quest_texts(d);
    let title = if installed {
        title
    } else if ready && d.quest_info.is_some() {
        "Echo VR is not installed".into()
    } else if ready {
        title
    } else {
        "Connect your Quest".into()
    };
    header(kit, &title, HEADER_Y);
    para(kit, &sub, TEXT_Y + 6.0, 14.0, style::TEXT);

    let mut open = None;
    let busy = d.quest_conn.checking || d.quest_busy;
    if installed {
        if big(
            kit,
            "q-launch",
            "Play on Quest",
            !d.quest_busy,
            "Start Echo VR on the headset",
        ) {
            quest_launch(d, ctx);
        }
    } else if ready && d.quest_info.is_some() {
        if big(
            kit,
            "q-install",
            "Install on Quest",
            true,
            "Install Echo VR on your Quest",
        ) {
            open = Some(Open::QuestInstall);
        }
    } else if big(
        kit,
        "q-connect",
        "Connect Quest",
        !busy,
        "Look for your Quest over USB",
    ) {
        d.check_quest(ctx, true);
    }

    let gap = 12.0;
    let x0 = CX - (2.0 * Btn::Middle.w() + gap) / 2.0;
    if installed {
        if kit.button(
            "q-stop",
            Btn::Middle,
            "Stop Echo VR",
            14.0,
            x0,
            ROW_Y,
            !d.quest_busy,
            "Close Echo VR on the headset",
        ) {
            quest_stop(d, ctx);
        }
    } else if kit.button(
        "q-install2",
        Btn::Middle,
        "Install on Quest",
        14.0,
        x0,
        ROW_Y,
        true,
        "Opens the Quest install wizard: APK and game data, patched if you need it",
    ) {
        open = Some(Open::QuestInstall);
    }
    if kit.button(
        "q-update",
        Btn::Middle,
        "Update on Quest",
        14.0,
        x0 + Btn::Middle.w() + gap,
        ROW_Y,
        true,
        "Opens the Quest update wizard: copies changed files to the headset",
    ) {
        open = Some(Open::QuestUpdate);
    }
    para(
        kit,
        "USB installs and launching need developer mode on the headset.",
        LOBBY_Y + 4.0,
        12.0,
        style::TEXT_DIM,
    );
    kit.link(
        "q-dev",
        "How to enable developer mode",
        DEV_MODE_URL,
        12.0,
        CX,
        LOBBY_Y + 28.0,
    );
    open
}
