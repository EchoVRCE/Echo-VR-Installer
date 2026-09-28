//! Play page: the selected version over the game art with a split PLAY button (its menu
//! picks any version, installed or not), the launch mode and options, and a row of cards
//! (join lobby, updates, Quest). The Quest variant starts Echo on the headset.

use egui::{pos2, vec2, Order, Rect, Sense};

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
/// The hero: title, sub line, the action row and a hint line under it.
const HERO_Y: f32 = 214.0;
const ROW_Y: f32 = HERO_Y + 92.0;
const HINT_Y: f32 = ROW_Y + style::BIG + 16.0;
const SPLIT_W: f32 = 290.0;
const CARDS_Y: f32 = 508.0;
const CARD_H: f32 = 168.0;

pub(super) fn show(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    let sel = if d.play_platform == Platform::Pc {
        0
    } else {
        1
    };
    if let Some(i) = kit.segmented(
        "play-platform",
        &["PC", "Quest"],
        sel,
        X0,
        64.0,
        190.0,
        30.0,
    ) {
        d.play_platform = if i == 0 {
            Platform::Pc
        } else {
            Platform::Quest
        };
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

fn hero_text(kit: &Kit, title: &str, sub: &str, sub_color: egui::Color32) {
    kit.text_fit(X0, HERO_Y, 860.0, title, style::display(32.0), style::TEXT);
    kit.text_fit(X0, HERO_Y + 52.0, 860.0, sub, style::body(15.0), sub_color);
}

fn card_x(i: usize) -> (f32, f32) {
    let w = ((CW - 32.0) / 3.0).floor();
    (X0 + i as f32 * (w + 16.0), w)
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

// ---- PC ----

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

/// What the main part of the split button does.
enum Main {
    Play,
    Stop,
    Install(VersionEntry),
    Reinstall(VersionEntry),
    Patch(String),
    SetUpRevive,
    Nothing,
}

fn pc(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    let target = d.target();
    let id = match &target {
        Target::Installed(v) | Target::Missing(v) => Some(v.id.clone()),
        Target::Available(e) => Some(e.id.clone()),
        Target::None => None,
    };
    let job_id = id.clone().filter(|id| d.jobs.contains_key(id)).or_else(|| {
        d.jobs
            .contains_key(setup::REVIVE_JOB)
            .then(|| setup::REVIVE_JOB.into())
    });
    let job = job_id
        .as_ref()
        .and_then(|id| d.jobs.get(id))
        .map(|j| (j.label.clone(), j.fraction));
    let needs_revive = d.state.profile.runtime == Runtime::Revive
        && (cfg!(windows) || d.demo)
        && d.revive_dir().is_none();
    let running = d.game().is_running();
    let ours = d.child.is_some();

    // Hero text and the main action.
    let danger = style::DANGER;
    let (label, sub_label, main, enabled, hint): (String, Option<String>, Main, bool, String) =
        match &target {
            Target::Installed(v) => {
                hero_text(
                    kit,
                    &v.name,
                    &format!("{}   —   {}", subline(d), v.root),
                    style::TEXT_DIM,
                );
                let lobby_ok = d.state.last_lobby.trim().is_empty()
                    || launch::lobby_uuid(&d.state.last_lobby).is_some();
                if running && ours {
                    ("STOP".into(), None, Main::Stop, true, String::new())
                } else if running {
                    (
                        "RUNNING".into(),
                        None,
                        Main::Nothing,
                        false,
                        "Echo VR was started outside the launcher.".into(),
                    )
                } else if d.state.owner == Some(false) && !v.patched {
                    (
                        "PATCH".into(),
                        Some("Licence patch".into()),
                        Main::Patch(v.id.clone()),
                        !d.any_job(),
                        "New players need a personal licence patch: authorize with Discord to get yours."
                            .into(),
                    )
                } else if needs_revive {
                    (
                        "SET UP STEAMVR".into(),
                        Some("Installs Revive".into()),
                        Main::SetUpRevive,
                        !d.any_job(),
                        "SteamVR (Revive) isn't installed yet. Setting it up asks for administrator rights."
                            .into(),
                    )
                } else {
                    ("PLAY".into(), None, Main::Play, lobby_ok, String::new())
                }
            }
            Target::Missing(v) => {
                hero_text(
                    kit,
                    &v.name,
                    &format!("Game files missing   —   {}", v.root),
                    danger,
                );
                let entry = v.catalog_id.as_ref().and_then(|cid| {
                    d.catalog
                        .as_ref()
                        .and_then(|c| c.pc().find(|e| &e.id == cid).cloned())
                });
                match entry {
                    Some(e) if !v.external => (
                        "REINSTALL".into(),
                        e.size.map(gb),
                        Main::Reinstall(e),
                        true,
                        String::new(),
                    ),
                    _ => (
                        "PLAY".into(),
                        None,
                        Main::Nothing,
                        false,
                        "The folder is gone. Pick its new location with ▾ → Add existing folder."
                            .into(),
                    ),
                }
            }
            Target::Available(e) => {
                let root = versions_root(d, &e.id);
                let size = e.size.map(gb).unwrap_or_else(|| "size unknown".into());
                let state = if job.is_some() {
                    "Installing"
                } else {
                    "Not installed"
                };
                hero_text(
                    kit,
                    &e.name,
                    &format!("{state}  ·  {size}   —   into {root}"),
                    style::TEXT_DIM,
                );
                let free = d.free_bytes();
                let short = matches!((e.size, free), (Some(need), Some(free)) if need > free);
                let hint = match (short, free) {
                    (true, Some(free)) => format!(
                        "Not enough space: needs {}, {} free. Pick another library in Settings.",
                        gb(e.size.unwrap_or_default()),
                        gb(free)
                    ),
                    _ if d.state.versions.is_empty() => {
                        "Already have Echo VR? Add its folder or find your Meta install with ▾."
                            .into()
                    }
                    _ => String::new(),
                };
                (
                    "INSTALL".into(),
                    e.size.map(gb),
                    Main::Install(e.clone()),
                    !short && !d.any_job(),
                    hint,
                )
            }
            Target::None => {
                hero_text(
                    kit,
                    "No versions yet",
                    "The version list is loading. You can also add a folder you already have with ▾.",
                    style::TEXT_DIM,
                );
                ("PLAY".into(), None, Main::Nothing, false, String::new())
            }
        };

    // The action row: [ PLAY | ▾ ], launch mode, options.
    let split_key = "play-split";
    let menu_rect;
    if let Some((jl, fraction)) = &job {
        kit.progress(X0, ROW_Y + 8.0, SPLIT_W, *fraction, jl);
        menu_rect = kit.rect(X0, ROW_Y, SPLIT_W, style::BIG);
        let id = job_id.clone().unwrap_or_default();
        if kit
            .flat_button(
                "job-cancel",
                Variant::Ghost,
                None,
                "Cancel",
                X0,
                HINT_Y,
                120.0,
                style::SMALL,
                true,
                "Stop this job",
            )
            .clicked
        {
            d.cancel_job(&id);
        }
    } else {
        let tip = match &main {
            Main::Play => "Start Echo VR",
            Main::Stop => "Close Echo VR",
            Main::Install(_) => "Download and install this version into your library",
            Main::Reinstall(_) => "Download this version again into its folder",
            Main::Patch(_) => "Opens Discord in your browser to get your personal patch",
            Main::SetUpRevive => "Download and install Revive, which runs Echo VR on SteamVR",
            Main::Nothing => "",
        };
        let split = kit.split_button(
            split_key,
            &label,
            sub_label.as_deref(),
            X0,
            ROW_Y,
            SPLIT_W,
            enabled,
            tip,
            "Choose a version: installed ones, or ones you can install",
        );
        menu_rect = split.menu_rect;
        if split.main {
            match main {
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
        if !hint.is_empty() {
            let color = if hint.starts_with("Not enough") || hint.starts_with("The folder") {
                danger
            } else {
                style::TEXT_MUTED
            };
            kit.text_fit(X0, HINT_Y + 4.0, 860.0, &hint, style::body(13.0), color);
        }
    }
    version_menu(d, kit, ctx, split_key, menu_rect);

    let mid_y = ROW_Y + (style::BIG - style::MID) / 2.0;
    let modes: Vec<String> = Runtime::ALL.iter().map(|r| r.label().to_string()).collect();
    let msel = Runtime::ALL
        .iter()
        .position(|r| *r == d.state.profile.runtime)
        .unwrap_or(0);
    if let Some(i) = kit.dropdown(
        "mode",
        &modes,
        msel,
        X0 + SPLIT_W + 16.0,
        mid_y,
        240.0,
        style::MID,
        "How Echo VR reaches your headset",
    ) {
        d.state.profile.runtime = Runtime::ALL[i];
        d.save();
    }
    let ox = X0 + SPLIT_W + 16.0 + 240.0 + 12.0;
    if kit
        .icon_button(
            "options",
            Icon::More,
            ox,
            mid_y,
            style::MID,
            "Launch options",
        )
        .clicked
    {
        d.options_open = !d.options_open;
    } else if d.options_open {
        let anchor = kit.rect(ox, mid_y, 50.0, style::MID);
        options_popover(d, kit, anchor);
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

/// The split button's menu: installed versions, versions to install, and actions.
fn version_menu(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context, key: &str, anchor: Rect) {
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
    let Some(i) = kit.menu_popup(key, anchor, w, &items) else {
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

fn lobby_card(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
    let (x, w) = card_x(0);
    kit.titled_card(x, CARDS_Y, w, CARD_H, "Join lobby");
    // A lobby link on the clipboard: one click to use it.
    if let Some(clip) = d.clip_lobby.clone() {
        let pw = kit.pill_width("Paste from clipboard", false);
        let px = x + w - 16.0 - pw;
        kit.pill(px, CARDS_Y + 12.0, "Paste from clipboard", style::OK, false);
        let r = kit.rect(px, CARDS_Y + 12.0, pw, 22.0);
        if kit.hot("lobby-clip", r, true, &clip).0.clicked {
            d.state.last_lobby = clip;
            d.clip_lobby = None;
            d.save();
        }
    }
    let invalid =
        !d.state.last_lobby.trim().is_empty() && launch::lobby_uuid(&d.state.last_lobby).is_none();
    let tip = "Starts Echo VR straight into this lobby (-lobbyid)";
    let bw = 110.0;
    if kit.input(
        "lobby",
        &mut d.state.last_lobby,
        x + 20.0,
        CARDS_Y + 50.0,
        w - 40.0 - bw - 10.0,
        style::MID,
        "spark:// link or lobby ID",
        invalid,
        tip,
    ) {
        d.save();
    }
    let running = d.game().is_running();
    let installed = matches!(d.target(), Target::Installed(_));
    let can = !invalid && !d.state.last_lobby.trim().is_empty() && !running && installed;
    if kit
        .flat_button(
            "join",
            Variant::Secondary,
            None,
            "Join",
            x + w - 20.0 - bw,
            CARDS_Y + 50.0,
            bw,
            style::MID,
            can,
            "Start Echo VR and join this lobby",
        )
        .clicked
    {
        try_start(d, ctx);
    }
    let (note, color) = if invalid {
        ("That doesn't look like a lobby link.", style::DANGER)
    } else if running {
        (
            "Echo VR is running -- join from the game for now.",
            style::TEXT_MUTED,
        )
    } else {
        (
            "Paste a lobby link to start straight into it.",
            style::TEXT_MUTED,
        )
    };
    kit.text_fit(
        x + 20.0,
        CARDS_Y + 100.0,
        w - 40.0,
        note,
        style::body(12.0),
        color,
    );
}

fn updates_card(
    d: &mut Dashboard,
    kit: &mut Kit,
    ctx: &egui::Context,
    v: Option<&InstalledVersion>,
) {
    let (x, w) = card_x(1);
    kit.titled_card(x, CARDS_Y, w, CARD_H, "Updates");
    let Some(v) = v else {
        kit.text_fit(
            x + 20.0,
            CARDS_Y + 50.0,
            w - 40.0,
            "Nothing to update yet",
            style::bold(16.0),
            style::TEXT,
        );
        kit.text_fit(
            x + 20.0,
            CARDS_Y + 74.0,
            w - 40.0,
            "Install the version first; updates run from here.",
            style::body(12.0),
            style::TEXT_MUTED,
        );
        return;
    };
    let note = d
        .update_note
        .get(&v.id)
        .cloned()
        .unwrap_or_else(|| "Community patches".into());
    kit.text_fit(
        x + 20.0,
        CARDS_Y + 50.0,
        w - 40.0,
        &note,
        style::bold(16.0),
        style::TEXT,
    );
    kit.text_fit(
        x + 20.0,
        CARDS_Y + 74.0,
        w - 40.0,
        "Keeps the game files of this version current.",
        style::body(12.0),
        style::TEXT_MUTED,
    );
    let by = CARDS_Y + CARD_H - 18.0 - style::MID;
    if let Some(j) = d.jobs.get(&v.id) {
        let (label, fraction) = (j.label.clone(), j.fraction);
        kit.progress(x + 20.0, by + 2.0, w - 150.0, fraction, &label);
        if kit
            .flat_button(
                "upd-cancel",
                Variant::Ghost,
                None,
                "Cancel",
                x + w - 120.0,
                by + (style::MID - style::SMALL) / 2.0,
                100.0,
                style::SMALL,
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
                by,
                260.0,
                style::MID,
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
        CARDS_Y + 50.0,
        w - 40.0,
        &title,
        style::bold(16.0),
        style::TEXT,
    );
    kit.text_fit(
        x + 20.0,
        CARDS_Y + 74.0,
        w - 40.0,
        &sub,
        style::body(12.0),
        style::TEXT_MUTED,
    );
    let by = CARDS_Y + CARD_H - 18.0 - style::MID;
    let installed = ready && d.quest_info.as_ref().is_some_and(|i| i.installed);
    if installed {
        let half = ((w - 52.0) / 2.0).floor();
        if kit
            .flat_button(
                "qc-launch",
                Variant::Secondary,
                Some(Icon::Play),
                "Launch",
                x + 20.0,
                by,
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
                Variant::Ghost,
                Some(Icon::Stop),
                "Stop",
                x + 32.0 + half,
                by,
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
                by,
                260.0,
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

fn quest_hero(d: &mut Dashboard, kit: &mut Kit, ctx: &egui::Context) {
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
    hero_text(kit, &title, &sub, style::TEXT_DIM);

    let job = d
        .jobs
        .get(setup::QUEST_JOB)
        .map(|j| (j.label.clone(), j.fraction));
    let busy = d.quest_conn.checking || d.quest_busy || d.any_job();
    let mid_y = ROW_Y + (style::BIG - style::MID) / 2.0;
    let side_x = X0 + SPLIT_W + 16.0;
    let mut hint = String::new();
    if let Some((label, fraction)) = job {
        kit.progress(X0, ROW_Y + 8.0, SPLIT_W, fraction, &label);
        if kit
            .flat_button(
                "q-cancel",
                Variant::Ghost,
                None,
                "Cancel",
                X0,
                HINT_Y,
                120.0,
                style::SMALL,
                true,
                "Stop after the current step",
            )
            .clicked
        {
            d.cancel_job(setup::QUEST_JOB);
        }
        kit.text_fit(
            X0 + 136.0,
            HINT_Y + 4.0,
            700.0,
            "Keep the headset connected until this finishes.",
            style::body(13.0),
            style::TEXT_MUTED,
        );
    } else if installed {
        if kit
            .flat_button(
                "q-launch",
                Variant::Primary,
                Some(Icon::Play),
                "PLAY ON QUEST",
                X0,
                ROW_Y,
                SPLIT_W,
                style::BIG,
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
                side_x,
                mid_y,
                150.0,
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
                side_x + 162.0,
                mid_y,
                170.0,
                style::MID,
                !busy,
                "Copy the latest game files to your Quest",
            )
            .clicked
        {
            setup::quest_update(d, ctx);
        }
    } else if ready && d.quest_info.is_some() {
        if kit
            .flat_button(
                "q-install",
                Variant::Primary,
                Some(Icon::Download),
                "INSTALL ON QUEST",
                X0,
                ROW_Y,
                SPLIT_W,
                style::BIG,
                !busy,
                "Download Echo VR and install it on your Quest",
            )
            .clicked
        {
            setup::ask_quest_install(d);
        }
        if d.state.owner == Some(false) {
            if kit
                .flat_button(
                    "q-link",
                    Variant::Secondary,
                    None,
                    "APK from a link…",
                    side_x,
                    mid_y,
                    230.0,
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
            hint = "New players get a personal patched APK through Discord.".into();
        }
    } else {
        if kit
            .flat_button(
                "q-connect",
                Variant::Primary,
                Some(Icon::Headset),
                "CONNECT",
                X0,
                ROW_Y,
                SPLIT_W,
                style::BIG,
                !d.quest_conn.checking && !d.quest_busy,
                "Look for your Quest over USB",
            )
            .clicked
        {
            d.check_quest(ctx, true);
        }
        hint = "Plug in your Quest by USB, with developer mode on.".into();
    }
    if !hint.is_empty() {
        kit.text_fit(
            X0,
            HINT_Y + 4.0,
            860.0,
            &hint,
            style::body(13.0),
            style::TEXT_MUTED,
        );
    }

    // Cards: install, update, help.
    let by = CARDS_Y + CARD_H - 18.0 - style::MID;
    let (x0, w) = card_x(0);
    kit.titled_card(x0, CARDS_Y, w, CARD_H, "Install");
    kit.text_fit(
        x0 + 20.0,
        CARDS_Y + 50.0,
        w - 40.0,
        if installed {
            "Reinstall"
        } else {
            "Fresh install"
        },
        style::bold(16.0),
        style::TEXT,
    );
    kit.text_fit(
        x0 + 20.0,
        CARDS_Y + 74.0,
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
            if installed {
                "Reinstall"
            } else {
                "Install on Quest"
            },
            x0 + 20.0,
            by,
            260.0,
            style::MID,
            !busy,
            "Download Echo VR and install it on your Quest over USB",
        )
        .clicked
    {
        setup::ask_quest_install(d);
    }
    let (x1, _) = card_x(1);
    kit.titled_card(x1, CARDS_Y, w, CARD_H, "Update");
    kit.text_fit(
        x1 + 20.0,
        CARDS_Y + 50.0,
        w - 40.0,
        "Community patches",
        style::bold(16.0),
        style::TEXT,
    );
    kit.text_fit(
        x1 + 20.0,
        CARDS_Y + 74.0,
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
            by,
            260.0,
            style::MID,
            !busy && (installed || !ready),
            "Copy the latest game files to your Quest",
        )
        .clicked
    {
        setup::quest_update(d, ctx);
    }
    let (x2, w2) = card_x(2);
    kit.titled_card(x2, CARDS_Y, w2, CARD_H, "Help");
    kit.text_fit(
        x2 + 20.0,
        CARDS_Y + 50.0,
        w2 - 40.0,
        "Developer mode",
        style::bold(16.0),
        style::TEXT,
    );
    kit.text_fit(
        x2 + 20.0,
        CARDS_Y + 74.0,
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
            by,
            260.0,
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
