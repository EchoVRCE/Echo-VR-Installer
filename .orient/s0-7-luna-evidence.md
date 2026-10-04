# S0-7 implementation evidence

Branch: `sprint/s0-7-play-split`, based on approved design `2db243b`.
Worktree: `.worktrees/s0-7-luna`.
Shared Cargo target: `/home/andrew/src/evr-launcher/.cargo-target`.

## Implementation

- Added the compact Play-only main/arrow/update controls; Install retains its existing hero row and switch geometry.
- Kept arrow boundaries at design x=300 and x=445, including inert seams; the job row divider is x=300.
- Moved the Play platform switch up to the Play row and lower Play content down to avoid overlap.
- Anchored the version menu under the arrow, constrained it to scroll, and close/disable it for busy or Quest states.
- Kept selected target name on line one and state/progress on line two. Long names ellide and expose the full value to hover and AccessKit.
- Added a Quest installing snapshot fixture and repeatable named viewport filtering.

## BAC-by-BAC evidence

- **BAC-0001 — Play/Install layout and old hotspot:** Headless renders inspected for Play normal/job and Install normal/job at 1280×720, 960×540 and 1680×720. Dedicated Quest normal/job renders were included at each size; the Quest chip/switch and install progress/cancel row remain in the compact layout without colliding with info/news. The actual old logo hotspot is clicked in `old_logo_hotspot_does_not_open_the_retired_easter_egg`, which asserts no dialog opens. `play_split_owns_only_its_polygon_and_keeps_both_seams_inert` probes the ownership boundary and inert seams. **Passed.** No automated pixel-overlap measurement; visual inspection is the layout check.
- **BAC-0002 — selection, persistence, accessibility, and scrolling:** `accesskit_arrow_selects_catalogue_and_main_routes_to_install` asserts the main and arrow are distinct accessible targets, activates the arrow without pointer input, selects A then B, verifies the closed-menu names and checked item, then activates main to Install. `empty_and_single_choice_menus_follow_the_contract` verifies zero choices have no arrow and the old arrow zone routes through the main action; one choice opens its checked entry plus Install another version. `non_demo_selection_saves_and_reloads_in_isolated_state_file` uses an isolated non-demo state file and reloads the selected id. `non_demo_selection_reports_a_failed_save` asserts the failure notification. `long_version_list_scrolls_below_arrow_at_named_sizes` scrolls a 24-entry list at all three named sizes and asserts scroll movement remains in range. `selected_catalogue_name_and_install_progress_are_primary`, `long_version_name_is_elided_without_dropping_state_or_progress`, and `elided_selected_name_exposes_the_full_name_on_hover` cover displayed state/progress, ellipsis, and full accessible/hover name. **Passed.**
- **BAC-0003 — launch and Install routes:** `installed_main_routes_the_selected_version_without_launching_a_different_one` chooses B, activates main, reaches the executable-missing preflight dialog, and asserts its path is B's selected root rather than A's; it does not launch a game. `missing_installed_target_routes_only_its_catalogue_id_to_install` covers a missing entry with a catalogue id and one without, asserting Install receives respectively `Some(id)` and `None`. The one-entry menu's Install another version action is covered in the BAC-0002 test. **Passed.**
- **BAC-0004 — open-menu transitions and independent controls:** `open_version_menu_clears_on_busy_and_quest_transitions` opens the menu then changes state through launch-start, external-running, owned-running, selected-target Install job, unrelated job, Quest, and back to PC. It asserts the menu closes, selected id remains, arrow is disabled while the owned-running STOP remains enabled, and job Cancel remains independently accessible/activatable. Quest has no arrow. PC/Quest job renders are the visual substitute for runtime progress layout. **Passed.**

## Observed red/green checks

- Geometry: changed arrow ownership from x=300.001 to x=301.001; the geometry test failed with observed `None` versus `Some(Arrow)`. Restored x=300.001 and the test passed.
- Selected name: temporarily removed selected catalog name from the primary info line; its test failed with `Installing` and `35%` in place of the expected selected name/state/progress. Restored; passed.
- Name fitting: temporarily disabled fitting; long-name test failed its ellipsis assertion. Restored fitting and corrected the overlong-name edge; passed.
- Arrow selection: temporarily disabled the arrow action; the AccessKit selection test failed because the version menu did not open. Restored; passed.
- One-entry popup: temporarily required more than one version before rendering the popup; the one-choice test failed with page `Play` where `Install` was expected after arrow activation. Restored `has_versions`; passed.
- Persistence: temporarily made the save call a no-op; isolated save/reload failed with `None` instead of `Some("pc-34.4")`. Restored; passed.
- Save error: temporarily changed the failure notification to a success message; the save-failure test failed its notice assertion. Restored; passed.
- Full long-name accessibility: before adding full-name AccessKit widget metadata, the tooltip test failed because querying the full name returned no node. Added accessible label and hover text; passed.

## Focused tests and renders

`CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test --bin EchoVR_Installer ui::launcher::play::split_info_tests -- --nocapture` — 11 passed.

Headless `headless_snapshots` with the named-shot filter passed at 1280×720, 960×540 and 1680×720. Renders include `launcher_play.png`, `launcher_play_installing.png`, `launcher_play_version_menu.png`, `launcher_play_quest.png`, `launcher_play_quest_installing.png`, `launcher_install.png`, and `launcher_install_installing.png`.

- 1280×720: `/tmp/s0-7-luna-final-1280/`
- 960×540: `/tmp/s0-7-luna-final-960/`
- 1680×720: `/tmp/s0-7-luna-final-1680/`

The menu render filename is `launcher_play_version_menu.png`.

## Final checks

- `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo fmt --all -- --check`: passed after formatting.
- `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo build --release`: passed (exit 0).
- `cargo test`: 177 passed, 1 failed, 13 ignored. The sole failure was the loopback test `core::remote_zip::tests::repairs_single_files_from_a_remote_zip`, panicking at `src/core/remote_zip.rs:187` with `PermissionDenied: Operation not permitted` while binding its local test server. All 11 Play split interaction tests passed in this run.
- `git diff --check`: passed.

## Review recheck: B1 Quest job progress

- `hero::play_job_row` now clips the progress fill to `compact_play_regions(arrow).0`. PC retains x=139..300 when its arrow is present; Quest and zero-choice PC use the full x=139..445 main button.
- The `QuestInstalling` headless fixture is now a known 100% job. Fixed screenshots were generated with `headless_snapshots` at all three named viewports and visually inspected:
  - `/tmp/s0-7-luna-b1-960/launcher_play_quest_installing.png` (960×540)
  - `/tmp/s0-7-luna-b1-1280/launcher_play_quest_installing.png` (1280×720)
  - `/tmp/s0-7-luna-b1-1680/launcher_play_quest_installing.png` (1680×720)
- Deliberately mutated `compact_job_progress_body` to return the old PC main bounds. `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test --bin EchoVR_Installer ui::launcher::hero::play_split_tests::job_progress_uses_the_active_main_button_width -- --nocapture` failed at the Quest-width assertion (actual 160.999 design pixels; expected 305.999). Restored the active-main-bounds implementation; the same test passed.
- Repeated the 100% Quest render with the old-width mutation to establish the visual red result:
  - `/tmp/s0-7-luna-b1-red-960/launcher_play_quest_installing.png`
  - `/tmp/s0-7-luna-b1-red-1280/launcher_play_quest_installing.png`
  - `/tmp/s0-7-luna-b1-red-1680/launcher_play_quest_installing.png`
  At 1280×720, the green fill stops at the PC split boundary while the 100% label remains visible; the fixed render fills the whole Quest button. Each headless render command exited 0.
