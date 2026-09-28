//! Play page: pick a version and launch mode (PC), or launch on the headset (Quest).

use super::{header, label, Dashboard, Msg, Page, PAGE_W};
use crate::core::error::UiError;
use crate::core::launcher::catalog::Platform;
use crate::core::launcher::store::Runtime;
use crate::core::launcher::{launch, quest};
use crate::core::{paths, revive};
use crate::ui::dialogs::Icon;
use crate::ui::kit::{Btn, Kit};
use crate::ui::launcher::Open;
use crate::ui::parts;
use crate::ui::theme;

const LAUNCH_ANYWAY: &str = "launch-anyway";

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    header(kit, "Play Echo VR");
    let mid = PAGE_W / 2.0;
    if kit.chip(
        "play-pc",
        "PC",
        12.0,
        mid - 130.0,
        52.0,
        120.0,
        26.0,
        d.play_platform == Platform::Pc,
        "Play Echo VR on your PC (Link, Virtual Desktop, SteamVR or flat)",
    ) {
        d.play_platform = Platform::Pc;
    }
    if kit.chip(
        "play-quest",
        "Quest",
        12.0,
        mid + 10.0,
        52.0,
        120.0,
        26.0,
        d.play_platform == Platform::Quest,
        "Start Echo VR on your Quest over USB",
    ) {
        d.play_platform = Platform::Quest;
    }
    match d.play_platform {
        Platform::Pc => pc(d, kit, ctx),
        Platform::Quest => quest_panel(d, kit, ctx),
    }
}

fn pc(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    // ---- version list (left) ----
    label(kit, 10.0, 90.0, "Version");
    let (lx, ly, lw, lh) = (10.0, 112.0, 470.0, 300.0);
    kit.round_box(
        lx,
        ly,
        lw,
        lh,
        10.0,
        theme::rgba(30, 20, 40, 150),
        Some(theme::BOX_BORDER),
    );
    if d.state.versions.is_empty() {
        kit.text_center(
            lx,
            ly + 60.0,
            lw,
            60.0,
            "No Echo VR version installed yet.",
            theme::arial(15.0),
            theme::WHITE,
            None,
        );
        if kit.button(
            "goto-versions",
            Btn::Middle,
            "Get Echo VR",
            14.0,
            lx + (lw - 212.0) / 2.0,
            ly + 140.0,
            true,
            "Install a version, or add an existing install",
        ) {
            d.page = Page::Versions;
        }
    } else {
        let row_h = 44.0;
        let content_h = d.state.versions.len() as f32 * row_h;
        kit.scroll(lx, ly, lw, lh, content_h + 8.0, &mut d.list_scroll);
        let selected = d.state.selected_version().map(|v| v.id.clone());
        let mut pick = None;
        let scroll = d.list_scroll;
        kit.clipped(lx + 2.0, ly + 2.0, lw - 4.0, lh - 4.0, |k| {
            for (i, v) in d.state.versions.iter().enumerate() {
                let y = ly + 4.0 + i as f32 * row_h - scroll;
                let is_sel = selected.as_deref() == Some(v.id.as_str());
                let c = k.area(
                    &format!("ver-{}", v.id),
                    lx + 4.0,
                    y,
                    lw - 8.0,
                    row_h - 4.0,
                    &format!("Play {}", v.name),
                );
                let bg = if is_sel {
                    theme::rgba(0, 180, 0, 160)
                } else if c.hovered {
                    theme::rgba(255, 255, 255, 40)
                } else {
                    theme::rgba(0, 0, 0, 0)
                };
                k.round_box(lx + 4.0, y, lw - 8.0, row_h - 4.0, 8.0, bg, None);
                k.text_left(
                    lx + 14.0,
                    y + 2.0,
                    20.0,
                    &v.name,
                    theme::conthrax(13.0),
                    theme::WHITE,
                );
                let ok = paths::has_echo_install(&v.root);
                let sub = if ok {
                    v.root.clone()
                } else {
                    format!("{} (missing!)", v.root)
                };
                let color = if ok {
                    theme::LIGHT_GRAY
                } else {
                    theme::MARK_BAD
                };
                k.text_left(lx + 14.0, y + 21.0, 16.0, &sub, theme::arial(11.0), color);
                if c.clicked {
                    pick = Some(v.id.clone());
                }
            }
        });
        if let Some(id) = pick {
            d.state.selected = Some(id);
            d.save();
        }
    }

    // ---- launch options (right) ----
    let rx = 500.0;
    label(kit, rx, 90.0, "Launch mode");
    let mut changed = false;
    for (i, rt) in Runtime::ALL.iter().enumerate() {
        let (x, y) = (rx + (i % 2) as f32 * 250.0, 112.0 + (i / 2) as f32 * 36.0);
        let tip = match rt {
            Runtime::MetaLink => "Quest Link / Air Link or a Rift, through the Meta Quest Link app",
            Runtime::VirtualDesktop => {
                "Stream to your Quest with Virtual Desktop (start its Streamer first)"
            }
            Runtime::Revive => "SteamVR headsets (Index, Vive, ...) through Revive",
            Runtime::Flat => "Play or spectate without a headset (-noovr)",
        };
        if kit.chip(
            &format!("rt-{i}"),
            rt.label(),
            12.0,
            x,
            y,
            240.0,
            30.0,
            d.state.profile.runtime == *rt,
            tip,
        ) {
            d.state.profile.runtime = *rt;
            changed = true;
        }
    }
    let flat = d.state.profile.runtime == Runtime::Flat;
    changed |= kit.checkbox(
        "spectator",
        &mut d.state.profile.spectator,
        "Spectator stream",
        12.0,
        rx,
        188.0,
        240.0,
        20.0,
        false,
        flat,
        "Flat mode only: start as a spectator (-spectatorstream)",
    );
    changed |= kit.checkbox(
        "windowed",
        &mut d.state.profile.windowed,
        "Windowed",
        12.0,
        rx + 250.0,
        188.0,
        240.0,
        20.0,
        false,
        true,
        "Run in a window instead of fullscreen (-windowed)",
    );
    label(kit, rx, 216.0, "Extra arguments");
    let r = kit.text_field(
        "extra-args",
        &mut d.state.profile.extra_args,
        rx,
        238.0,
        490.0,
        24.0,
        12.0,
        "e.g. -mp -http",
        theme::FIELD_BG,
        "Additional command-line arguments for echovr.exe",
    );
    changed |= r.committed;
    label(kit, rx, 270.0, "Join lobby (optional)");
    let lobby_valid =
        d.state.last_lobby.trim().is_empty() || launch::lobby_uuid(&d.state.last_lobby).is_some();
    let bg = if lobby_valid {
        theme::FIELD_BG
    } else {
        theme::FIELD_BG_INVALID
    };
    let r = kit.text_field(
        "lobby",
        &mut d.state.last_lobby,
        rx,
        292.0,
        490.0,
        24.0,
        12.0,
        "Paste a spark:// link or lobby ID",
        bg,
        "Starts Echo VR straight into this lobby (-lobbyid)",
    );
    changed |= r.committed;
    if changed {
        d.save();
    }

    // ---- play / stop ----
    let game = d.game();
    let bx = rx + (490.0 - Btn::Big.w()) / 2.0;
    if game.is_running() {
        let ours = d.child.is_some();
        let tip = if ours {
            "Close Echo VR"
        } else {
            "Echo VR was started outside the launcher"
        };
        if kit.button(
            "stop",
            Btn::Big,
            if ours { "Stop Echo VR" } else { "Running..." },
            20.0,
            bx,
            350.0,
            ours,
            tip,
        ) {
            if let Some(mut c) = d.child.take() {
                let _ = c.kill();
            }
        }
    } else {
        let can = d.state.selected_version().is_some() && lobby_valid;
        if kit.button(
            "play",
            Btn::Big,
            "PLAY",
            24.0,
            bx,
            350.0,
            can,
            "Start Echo VR with these settings",
        ) {
            match launch::preflight(&d.state.profile) {
                Some(w) => d.dialogs.confirm(
                    LAUNCH_ANYWAY,
                    "Launch Echo VR",
                    &format!("{w}\n\nLaunch anyway?"),
                    Icon::Warning,
                ),
                None => start(d),
            }
        }
    }
    if d.dialogs.take(LAUNCH_ANYWAY).is_some_and(|a| a.is_yes()) {
        start(d);
    }
    let _ = ctx;
    None
}

fn start(d: &mut Dashboard) {
    let Some(v) = d.state.selected_version().cloned() else {
        return;
    };
    let exe = paths::exe_path(&v.root);
    if !exe.is_file() {
        d.dialogs.error("Echo VR not found", &format!("echovr.exe is missing in {}.\nRepair or reinstall this version on the Versions page.", v.root), Default::default());
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

fn quest_panel(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) -> Option<Open> {
    if !d.quest_conn_started {
        d.quest_conn_started = true;
        d.quest_conn.check(ctx, false);
    }
    d.quest_conn.draw(kit, 100.0, PAGE_W);
    let mid = PAGE_W / 2.0;
    let busy = d.quest_busy || d.quest_conn.checking;
    if kit.button(
        "q-connect",
        Btn::Middle,
        "Connect to Quest",
        14.0,
        mid - 106.0,
        130.0,
        !busy,
        "Check that your Quest is connected and has allowed this PC",
    ) {
        d.quest_conn.check(ctx, true);
        d.quest_info = None;
    }
    let ready = d.quest_conn.status == Some(crate::core::adb::devices::Status::Ready);
    if ready && d.quest_info.is_none() && !d.quest_busy {
        d.quest_busy = true;
        d.worker.spawn(ctx, |tx| {
            tx.send(Msg::QuestInfo(
                quest::info().map_err(|e| UiError::from_anyhow(&e, "Quest")),
            ))
        });
    }
    let info = match (&d.quest_info, ready) {
        (Some(i), true) => i.version_label(),
        (None, true) => "Reading the installed version...".into(),
        _ => "Connect your Quest to see the installed version".into(),
    };
    parts::progress_label(kit, &info, 13.0, mid - 300.0, 182.0, 600.0, 28.0);

    let installed = ready && d.quest_info.as_ref().is_some_and(|i| i.installed);
    if kit.button(
        "q-launch",
        Btn::Big,
        "Launch on Quest",
        18.0,
        mid - 290.0,
        228.0,
        installed && !d.quest_busy,
        "Start Echo VR on the headset",
    ) {
        d.quest_busy = true;
        d.worker.spawn(ctx, |tx| {
            tx.send(Msg::QuestAction(quest::launch().map_err(|e| {
                UiError::from_anyhow(&e, "Couldn't start Echo VR")
            })))
        });
    }
    if kit.button(
        "q-stop",
        Btn::Big,
        "Stop on Quest",
        18.0,
        mid + 8.0,
        228.0,
        installed && !d.quest_busy,
        "Close Echo VR on the headset",
    ) {
        d.quest_busy = true;
        d.worker.spawn(ctx, |tx| {
            tx.send(Msg::QuestAction(
                quest::stop().map_err(|e| UiError::from_anyhow(&e, "Couldn't stop Echo VR")),
            ))
        });
    }
    let mut open = None;
    if kit.button(
        "q-install",
        Btn::Middle,
        "Install on Quest",
        14.0,
        mid - 220.0,
        300.0,
        true,
        "Install Echo VR on your Quest (wizard)",
    ) {
        open = Some(Open::QuestInstall);
    }
    if kit.button(
        "q-update",
        Btn::Middle,
        "Update on Quest",
        14.0,
        mid + 8.0,
        300.0,
        true,
        "Update Echo VR on your Quest (wizard)",
    ) {
        open = Some(Open::QuestUpdate);
    }
    open
}
