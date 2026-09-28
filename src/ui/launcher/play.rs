//! Play page: a hero block (version, launch mode, PLAY) over the game art and a row of
//! cards (join lobby, updates, Quest). The Quest variant starts Echo on the headset.

use egui::{pos2, vec2, Order, Rect, Sense};

use super::{versions, Dashboard, Msg, CW, X0};
use crate::core::adb::devices::Status;
use crate::core::error::UiError;
use crate::core::launcher::catalog::Platform;
use crate::core::launcher::store::{InstalledVersion, Runtime};
use crate::core::launcher::{launch, quest};
use crate::core::{paths, revive};
use crate::ui::dialogs::Icon as DlgIcon;
use crate::ui::kit::Kit;
use crate::ui::launcher::Open;
use crate::ui::style::{self, Icon, Variant};

const LAUNCH_ANYWAY: &str = "launch-anyway";
const HERO_Y: f32 = 250.0;
const CARDS_Y: f32 = 512.0;
const CARD_H: f32 = 176.0;

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    let sel = if d.play_platform == Platform::Pc {
        0
    } else {
        1
    };
    if let Some(i) = kit.segmented(
        "play-platform",
        &["PC", "Quest"],
        sel,
        X0 + 84.0,
        17.0,
        170.0,
        30.0,
    ) {
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
    if d.dialogs.take(LAUNCH_ANYWAY).is_some_and(|a| a.is_yes()) {
        start(d);
    }
    match d.play_platform {
        Platform::Pc if d.state.versions.is_empty() => welcome(d, kit, ctx),
        Platform::Pc => pc(d, kit, ctx),
        Platform::Quest => quest_hero(d, kit, ctx),
    }
}

fn hero_text(kit: &Kit, kicker: &str, kicker_color: egui::Color32, title: &str, sub: &str) {
    kit.caps(X0, HERO_Y, kicker, kicker_color);
    kit.text_fit(
        X0,
        HERO_Y + 22.0,
        760.0,
        title,
        style::display(32.0),
        style::TEXT,
    );
    kit.text_fit(
        X0,
        HERO_Y + 70.0,
        760.0,
        sub,
        style::body(15.0),
        style::TEXT_DIM,
    );
}

fn card_x(i: usize) -> (f32, f32) {
    let w = ((CW - 32.0) / 3.0).floor();
    (X0 + i as f32 * (w + 16.0), w)
}

// ---- PC ----

fn welcome(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    hero_text(
        kit,
        "Welcome",
        style::ACCENT,
        "Get Echo VR",
        "Install the game, or add an install that is already on this PC.",
    );
    let y = HERO_Y + 112.0;
    let entry = d.catalog.as_ref().and_then(|c| c.pc().next().cloned());
    let job = entry
        .as_ref()
        .and_then(|e| d.jobs.get(&e.id))
        .map(|j| (j.label.clone(), j.fraction));
    let mut open = None;
    if let (Some((label, fraction)), Some(e)) = (job, entry.as_ref()) {
        kit.progress(X0, y + 6.0, 420.0, fraction, &label);
        if kit
            .flat_button(
                "welcome-cancel",
                Variant::Ghost,
                None,
                "Cancel",
                X0 + 436.0,
                y + 8.0,
                96.0,
                36.0,
                true,
                "Stop the download",
            )
            .clicked
        {
            d.cancel_job(&e.id);
        }
    } else {
        let tip = "Download and install the current Echo VR build into your library";
        if kit
            .flat_button(
                "welcome-install",
                Variant::Primary,
                Some(Icon::Download),
                "INSTALL",
                X0,
                y,
                220.0,
                56.0,
                entry.is_some(),
                tip,
            )
            .clicked
        {
            if let Some(e) = entry {
                versions::install(d, ctx, e);
            }
        }
        if kit
            .flat_button(
                "welcome-add",
                Variant::Secondary,
                Some(Icon::Folder),
                "Add existing folder",
                X0 + 236.0,
                y + 6.0,
                210.0,
                44.0,
                true,
                "Use an Echo VR install that is already on this PC",
            )
            .clicked
        {
            versions::add_existing(d);
        }
        if kit
            .flat_button(
                "welcome-classic",
                Variant::Ghost,
                None,
                "Classic installer",
                X0 + 462.0,
                y + 6.0,
                170.0,
                44.0,
                true,
                "The step-by-step installer (licence patch, Revive setup)",
            )
            .clicked
        {
            open = Some(Open::PcInstall);
        }
    }
    quest_card(d, kit, ctx, 0);
    open
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
    let (kicker, kc) = if !present {
        ("Missing files", style::DANGER)
    } else if v.external {
        ("PC  ·  Existing install", style::ACCENT)
    } else {
        ("PC  ·  Ready", style::ACCENT)
    };
    hero_text(
        kit,
        kicker,
        kc,
        &v.name,
        &format!("{}   —   {}", subline(d), v.root),
    );

    // Action row.
    let y = HERO_Y + 112.0;
    if d.game().is_running() {
        let ours = d.child.is_some();
        let tip = if ours {
            "Close Echo VR"
        } else {
            "Echo VR was started outside the launcher"
        };
        let label = if ours { "STOP" } else { "RUNNING" };
        if kit
            .flat_button(
                "stop",
                Variant::Danger,
                Some(Icon::Stop),
                label,
                X0,
                y,
                220.0,
                56.0,
                ours,
                tip,
            )
            .clicked
        {
            if let Some(mut c) = d.child.take() {
                let _ = c.kill();
            }
        }
    } else {
        let lobby_ok = d.state.last_lobby.trim().is_empty()
            || launch::lobby_uuid(&d.state.last_lobby).is_some();
        if kit
            .flat_button(
                "play",
                Variant::Primary,
                Some(Icon::Play),
                "PLAY",
                X0,
                y,
                220.0,
                56.0,
                present && lobby_ok,
                "Start Echo VR",
            )
            .clicked
        {
            try_start(d);
        }
    }
    let names: Vec<String> = d.state.versions.iter().map(|v| v.name.clone()).collect();
    let sel = d
        .state
        .versions
        .iter()
        .position(|x| x.id == v.id)
        .unwrap_or(0);
    if let Some(i) = kit.dropdown(
        "version",
        &names,
        sel,
        X0 + 236.0,
        y + 6.0,
        280.0,
        44.0,
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
        &modes,
        msel,
        X0 + 528.0,
        y + 6.0,
        220.0,
        44.0,
        "How Echo VR reaches your headset",
    ) {
        d.state.profile.runtime = Runtime::ALL[i];
        d.save();
    }
    if kit
        .icon_button(
            "options",
            Icon::More,
            X0 + 760.0,
            y + 6.0,
            44.0,
            "Launch options",
        )
        .clicked
    {
        d.options_open = !d.options_open;
    } else if d.options_open {
        let anchor = kit.rect(X0 + 760.0, y + 6.0, 44.0, 44.0);
        options_popover(d, kit, anchor);
    }

    lobby_card(d, kit);
    updates_card(d, kit, ctx, &v);
    quest_card(d, kit, ctx, 2);
    None
}

/// Launch options in a floating card next to the `⋯` button.
fn options_popover(d: &mut Dashboard, kit: &mut Kit, anchor: Rect) {
    let (w, h) = (380.0, 206.0);
    let pos = pos2(anchor.max.x + 12.0, anchor.min.y - 8.0);
    let popup = Rect::from_min_size(pos, vec2(w, h));
    let ctx = kit.ctx();
    let assets = kit.assets;
    let mut changed = false;
    egui::Area::new(egui::Id::new("launch-options"))
        .order(Order::Foreground)
        .fixed_pos(pos)
        .show(&ctx, |ui| {
            ui.allocate_exact_size(vec2(w, h), Sense::hover());
            let mut k = Kit::new(ui, assets, "launch-options", false);
            k.origin = pos;
            let p = k.ui.painter();
            p.rect_filled(
                popup.translate(vec2(0.0, 4.0)).expand(2.0),
                10.0,
                style::with_alpha(egui::Color32::BLACK, 90),
            );
            p.rect_filled(popup, 10.0, style::SURFACE_SOLID);
            p.rect_stroke(
                popup,
                10.0,
                egui::Stroke::new(1.0, style::BORDER_HI),
                egui::StrokeKind::Inside,
            );
            k.caps(20.0, 18.0, "Launch options", style::TEXT_MUTED);
            let flat = d.state.profile.runtime == Runtime::Flat;
            changed |= k.toggle(
                "opt-windowed",
                &mut d.state.profile.windowed,
                "Windowed",
                20.0,
                44.0,
                true,
                "Run in a window (-windowed)",
            );
            changed |= k.toggle(
                "opt-spectator",
                &mut d.state.profile.spectator,
                "Spectator stream",
                20.0,
                76.0,
                flat,
                "Flat mode only: join as a spectator (-spectatorstream)",
            );
            k.caps(20.0, 116.0, "Extra arguments", style::TEXT_MUTED);
            changed |= k.input(
                "opt-args",
                &mut d.state.profile.extra_args,
                20.0,
                138.0,
                w - 40.0,
                40.0,
                "e.g. -mp -http",
                false,
                "Additional arguments for echovr.exe",
            );
        });
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

fn lobby_card(d: &mut Dashboard, kit: &mut Kit) {
    let (x, w) = card_x(0);
    kit.titled_card(x, CARDS_Y, w, CARD_H, "Join lobby");
    let invalid =
        !d.state.last_lobby.trim().is_empty() && launch::lobby_uuid(&d.state.last_lobby).is_none();
    let tip = "Starts Echo VR straight into this lobby (-lobbyid)";
    if kit.input(
        "lobby",
        &mut d.state.last_lobby,
        x + 20.0,
        CARDS_Y + 46.0,
        w - 40.0,
        40.0,
        "Paste a spark:// link or lobby ID",
        invalid,
        tip,
    ) {
        d.save();
    }
    let running = d.game().is_running();
    let note = if invalid {
        "That doesn't look like a lobby link."
    } else if running {
        "Echo VR is running -- join from the game for now."
    } else {
        "The server browser arrives with the Servers page."
    };
    kit.text_fit(
        x + 20.0,
        CARDS_Y + 94.0,
        w - 40.0,
        note,
        style::body(12.0),
        if invalid {
            style::DANGER
        } else {
            style::TEXT_MUTED
        },
    );
    let can = !invalid && !d.state.last_lobby.trim().is_empty() && !running;
    if kit
        .flat_button(
            "join",
            Variant::Secondary,
            Some(Icon::Play),
            "Launch into lobby",
            x + 20.0,
            CARDS_Y + 120.0,
            w - 40.0,
            40.0,
            can,
            "Start Echo VR and join this lobby",
        )
        .clicked
    {
        try_start(d);
    }
}

fn updates_card(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context, v: &InstalledVersion) {
    let (x, w) = card_x(1);
    kit.titled_card(x, CARDS_Y, w, CARD_H, "Updates");
    let note = d
        .update_note
        .get(&v.id)
        .cloned()
        .unwrap_or_else(|| "Community patches".into());
    kit.text_fit(
        x + 20.0,
        CARDS_Y + 46.0,
        w - 40.0,
        &note,
        style::bold(16.0),
        style::TEXT,
    );
    kit.text_fit(
        x + 20.0,
        CARDS_Y + 70.0,
        w - 40.0,
        "Keeps the game files of this version current.",
        style::body(12.0),
        style::TEXT_MUTED,
    );
    if let Some(j) = d.jobs.get(&v.id) {
        let (label, fraction) = (j.label.clone(), j.fraction);
        kit.progress(x + 20.0, CARDS_Y + 94.0, w - 136.0, fraction, &label);
        if kit
            .flat_button(
                "upd-cancel",
                Variant::Ghost,
                None,
                "Cancel",
                x + w - 108.0,
                CARDS_Y + 104.0,
                88.0,
                32.0,
                true,
                "Stop",
            )
            .clicked
        {
            d.cancel_job(&v.id);
        }
    } else {
        let ok = (d.demo || paths::has_echo_install(&v.root)) && !d.game().is_running();
        if kit
            .flat_button(
                "upd-check",
                Variant::Secondary,
                Some(Icon::Refresh),
                "Check for updates",
                x + 20.0,
                CARDS_Y + 120.0,
                w - 40.0,
                40.0,
                ok,
                "Download any changed game files",
            )
            .clicked
        {
            versions::update(d, ctx, v.clone());
        }
    }
}

fn quest_card(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context, slot: usize) {
    let (x, w) = card_x(slot);
    kit.titled_card(x, CARDS_Y, w, CARD_H, "Quest");
    let ready = d.quest_conn.status == Some(Status::Ready);
    let (title, sub) = quest_texts(d);
    kit.text_fit(
        x + 20.0,
        CARDS_Y + 46.0,
        w - 40.0,
        &title,
        style::bold(16.0),
        style::TEXT,
    );
    kit.text_fit(
        x + 20.0,
        CARDS_Y + 70.0,
        w - 40.0,
        &sub,
        style::body(12.0),
        style::TEXT_MUTED,
    );
    let installed = ready && d.quest_info.as_ref().is_some_and(|i| i.installed);
    if installed {
        let half = (w - 48.0) / 2.0;
        if kit
            .flat_button(
                "qc-launch",
                Variant::Secondary,
                Some(Icon::Play),
                "Launch",
                x + 20.0,
                CARDS_Y + 120.0,
                half,
                40.0,
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
                Variant::Ghost,
                Some(Icon::Stop),
                "Stop",
                x + 28.0 + half,
                CARDS_Y + 120.0,
                half,
                40.0,
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
                CARDS_Y + 120.0,
                w - 40.0,
                40.0,
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

fn quest_hero(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
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
    hero_text(kit, "Quest", style::ACCENT, &title, &sub);

    let y = HERO_Y + 112.0;
    let mut open = None;
    if installed {
        if kit
            .flat_button(
                "q-launch",
                Variant::Primary,
                Some(Icon::Play),
                "LAUNCH",
                X0,
                y,
                220.0,
                56.0,
                !d.quest_busy,
                "Start Echo VR on the headset",
            )
            .clicked
        {
            quest_launch(d, ctx);
        }
        if kit
            .flat_button(
                "q-stop",
                Variant::Secondary,
                Some(Icon::Stop),
                "Stop",
                X0 + 236.0,
                y + 6.0,
                140.0,
                44.0,
                !d.quest_busy,
                "Close Echo VR on the headset",
            )
            .clicked
        {
            quest_stop(d, ctx);
        }
    } else if ready && d.quest_info.is_some() {
        if kit
            .flat_button(
                "q-install",
                Variant::Primary,
                Some(Icon::Download),
                "INSTALL",
                X0,
                y,
                220.0,
                56.0,
                true,
                "Install Echo VR on your Quest",
            )
            .clicked
        {
            open = Some(Open::QuestInstall);
        }
    } else {
        let busy = d.quest_conn.checking || d.quest_busy;
        if kit
            .flat_button(
                "q-connect",
                Variant::Primary,
                Some(Icon::Headset),
                "CONNECT",
                X0,
                y,
                220.0,
                56.0,
                !busy,
                "Look for your Quest over USB",
            )
            .clicked
        {
            d.check_quest(ctx, true);
        }
    }

    // Cards: install, update, help.
    let (x0, w) = card_x(0);
    kit.titled_card(x0, CARDS_Y, w, CARD_H, "Install");
    kit.text_fit(
        x0 + 20.0,
        CARDS_Y + 46.0,
        w - 40.0,
        "Fresh install",
        style::bold(16.0),
        style::TEXT,
    );
    kit.text_fit(
        x0 + 20.0,
        CARDS_Y + 70.0,
        w - 40.0,
        "APK and game data, patched if you need it.",
        style::body(12.0),
        style::TEXT_MUTED,
    );
    if kit
        .flat_button(
            "qc-install",
            Variant::Secondary,
            Some(Icon::Download),
            "Install on Quest",
            x0 + 20.0,
            CARDS_Y + 120.0,
            w - 40.0,
            40.0,
            true,
            "Opens the Quest install wizard",
        )
        .clicked
    {
        open = Some(Open::QuestInstall);
    }
    let (x1, _) = card_x(1);
    kit.titled_card(x1, CARDS_Y, w, CARD_H, "Update");
    kit.text_fit(
        x1 + 20.0,
        CARDS_Y + 46.0,
        w - 40.0,
        "Community patches",
        style::bold(16.0),
        style::TEXT,
    );
    kit.text_fit(
        x1 + 20.0,
        CARDS_Y + 70.0,
        w - 40.0,
        "Copies changed files to the headset.",
        style::body(12.0),
        style::TEXT_MUTED,
    );
    if kit
        .flat_button(
            "qc-update",
            Variant::Secondary,
            Some(Icon::Refresh),
            "Update on Quest",
            x1 + 20.0,
            CARDS_Y + 120.0,
            w - 40.0,
            40.0,
            true,
            "Opens the Quest update wizard",
        )
        .clicked
    {
        open = Some(Open::QuestUpdate);
    }
    let (x2, w2) = card_x(2);
    kit.titled_card(x2, CARDS_Y, w2, CARD_H, "Help");
    kit.text_fit(
        x2 + 20.0,
        CARDS_Y + 46.0,
        w2 - 40.0,
        "Developer mode",
        style::bold(16.0),
        style::TEXT,
    );
    kit.text_fit(
        x2 + 20.0,
        CARDS_Y + 70.0,
        w2 - 40.0,
        "Needed for USB installs and launching.",
        style::body(12.0),
        style::TEXT_MUTED,
    );
    if kit
        .flat_button(
            "qc-help",
            Variant::Ghost,
            Some(Icon::Info),
            "How to enable it",
            x2 + 20.0,
            CARDS_Y + 120.0,
            w2 - 40.0,
            40.0,
            true,
            "Opens a guide in your browser",
        )
        .clicked
    {
        crate::core::platform::open_url(
            "https://learn.adafruit.com/sideloading-on-oculus-quest/enable-developer-mode",
        );
    }
    open
}
