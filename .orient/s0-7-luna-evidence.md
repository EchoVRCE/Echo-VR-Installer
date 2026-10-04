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

- Fix and 100% Quest snapshot fixture committed as `5f21cef` (`Fix Quest Play job progress width`).
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
- Each render used `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target ECHOVR_SNAPSHOTS=<output-dir> ECHOVR_SNAPSHOTS_ONLY=launcher_play_quest_installing ECHOVR_SNAPSHOTS_SIZE=<width>x<height> ECHOVR_SNAPSHOTS_DEMO=1 cargo test --bin EchoVR_Installer headless_snapshots -- --ignored --nocapture` and exited 0 at 960×540, 1280×720, and 1680×720, for both the old-width mutation and fixed implementation.

## Review recheck: Quest info band and widget interaction evidence

- Removed the second Quest device append in `quest_action`. `quest_info` supplies the device suffix once for both ready and installing branches.
- `quest_info_band_includes_the_device_once_for_ready_and_installing` exercises both action states. With the old append restored deliberately, the focused test failed at the ready assertion with 2 device strings where 1 was expected; after restoring the fix, it passed for ready and installing (including 100% progress).
- Added widget-level coverage: `arrow_opens_with_keyboard_and_pointer_selects_a_catalogue_choice` focuses and activates the arrow with Enter, then selects the catalogue row with an actual pointer click; `open_version_menu_clears_on_busy_and_quest_transitions` attempts pointer and keyboard activation while the arrow is disabled in a busy game state and confirms the menu stays closed; `platform_switch_widgets_enter_quest_and_return_to_pc` activates the platform switch controls to Quest and back to PC and asserts the platform and arrow visibility after each action.
- Focused verification after these changes: `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test --bin EchoVR_Installer ui::launcher::play::split_info_tests -- --nocapture` — 14 passed; `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test --bin EchoVR_Installer ui::launcher::hero::play_split_tests -- --nocapture` — 3 passed. `cargo fmt --all -- --check` and `git diff --check` passed.
- Full checks on the combined implementation tree: `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo build --release` — exit 0; `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test` — exit 101, 181 passed, 1 failed, 13 ignored. The sole failure is the known `core::remote_zip::tests::repairs_single_files_from_a_remote_zip` loopback bind at `src/core/remote_zip.rs:187`, denied by the sandbox (`PermissionDenied: Operation not permitted`). Compared with the pinned `ad9f709` baseline of 165 passed/13 ignored, this tree has 16 additional passing tests and the same environment-limited loopback failure. `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo fmt --all -- --check` passed.

## Sol final review follow-up

- **B1 determinate and indeterminate clips:** `play_job_row` now obtains its body from the active main-button bounds and calls the same `job_progress_clip` for both fractions and the moving indeterminate band. The regression test asserts PC determinate and indeterminate clips stay below x=300; a 100% Quest determinate clip reaches the full Quest main bound, and the Quest indeterminate band extends past x=300 while remaining inside the full bound. Deliberately capped the indeterminate branch to the old PC width; the test failed at `quest_indeterminate.max.x > 300.0`. Restored the active-bound clip; test passes. The earlier old-width mutation also failed the Quest body-width assertion (299.999 vs 444.999).
- Re-rendered the known 100% Quest job from the fixed tree and visually inspected all sizes:
  - `/tmp/s0-7-luna-finalfix-960/launcher_play_quest_installing.png`
  - `/tmp/s0-7-luna-finalfix-1280/launcher_play_quest_installing.png`
  - `/tmp/s0-7-luna-finalfix-1680/launcher_play_quest_installing.png`
  All have full green fill to the Quest main-button edge. The test fixture is set to 100%, and `quest_info_band_includes_the_device_once_for_ready_and_installing` checks its 100% status.
- **B2 real inputs:** the arrow test sends separate pointer-move, primary-button-down and primary-button-up events to the arrow, then asserts the menu stays open. It uses focused Enter key events for arrow activation and main routing to Install. Removed the just-opened-frame guard deliberately; the pointer test failed because outside-click dismissal immediately closed the menu. Replaced click sensing with hover sensing deliberately; the keyboard-arrow assertion failed. Forced the main response to false deliberately; the keyboard-main check failed with page `Play` instead of `Install`. Restored all paths; the test passes.
- **B4 switch event:** `platform_switch_widgets_enter_quest_and_return_to_pc` opens the version menu, sends physical pointer events to the Quest switch, and asserts immediately after the release frame that the platform is Quest and the menu is closed. It then clicks PC and asserts the platform returns to PC. Forced the switch handler to select PC deliberately; this test failed with `Pc` instead of `Quest`. Restored the platform assignment; test passes.
- Final focused commands: `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test --bin EchoVR_Installer ui::launcher::play::split_info_tests -- --nocapture` — 14 passed; `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test --bin EchoVR_Installer ui::launcher::hero::play_split_tests -- --nocapture` — 3 passed. The final three `headless_snapshots` runs used the command recorded above with output dirs `s0-7-luna-finalfix-{960,1280,1680}`, matching `ECHOVR_SNAPSHOTS_SIZE` values, and each exited 0.
- Final `cargo build --release` exited 0; final `cargo test` exited 101 with 181 passed, the known sandbox loopback bind failure, and 13 ignored. Final `cargo fmt --all -- --check` passed. `B3` remains covered by the existing selected-version route and missing-installed-target tests; Sol found it resolved.

## Astra recheck follow-up

- `quest_info_band_includes_the_device_once_for_ready_and_installing` covers ready/not-installed Quest and the installing job. Both assert the device string occurs exactly once; the installing state also asserts its `100%` status. The `play_quest_installing` fixture is the known-100% Quest render.
- `open_version_menu_clears_on_busy_and_quest_transitions` now asserts selected ID after each BAC-0004 transition/action. Launching, external-running, owned-running, selected-target install, unrelated job, Quest switch, and return to PC retain the selection. Quest and PC transitions use real pointer click events on their platform switch controls while the menu is open.
- Busy disabled behavior is exercised with physical pointer and keyboard events on the arrow; the menu remains closed and tests assert the exact reason for launching, external/owned running, selected-target install, and unrelated jobs. `arrow_opens_with_keyboard_and_pointer_selects_a_catalogue_choice` retains separate physical keyboard arrow activation and physical pointer arrow activation paths.
- BAC-0003 now begins on installed A and selects installed B through a physical pointer click on the version menu row before main activation. The executable-missing preflight is still asserted against B's root.
- Added `play_quest_installing_42`, a reproducible 42% Quest install render. Render commands matching the 100% evidence passed at 960x540, 1280x720, and 1680x720. Outputs: `/tmp/s0-7-luna-finalfix-42-{960,1280,1680}/launcher_play_quest_installing_42.png`. The 100% outputs remain `/tmp/s0-7-luna-finalfix-{960,1280,1680}/launcher_play_quest_installing.png`. Both progress states remain confined to the full active Quest main-button bounds; B1 tests also assert PC remains capped at x<300.
- Follow-up focused verification: Play split tests 14 passed; hero split tests 3 passed; BAC-0004 focused rerun 1 passed. `cargo build --release` passed; `cargo fmt --all -- --check` and `git diff --check` passed.

## Independent review follow-up

- Installed BAC-0003 routing remains menu-driven: the test starts with A selected, uses a pointer event on the version menu row to select installed B, then activates main and asserts the not-found preflight contains B's distinct root and not A's.
- `physical_pointer_arrow_opens_only_the_menu` is a separate test. It derives the arrow center from its accessible rect, sends move/down/up in distinct passes, and asserts the opening release leaves the popup open while page, action, selection, jobs, update note, and dialogs remain unchanged. It then clicks outside on a later pass and asserts the popup closes while page/action/selection stay unchanged, preserving outside-click dismissal behavior.
- BAC-0004 now applies `assert_busy_arrow_rejected` to launch-start, external-running, owned-running, selected-target install, and unrelated job transitions. Each assertion checks disabled AccessKit metadata, hovers the disabled arrow's rect center for six frames, and checks the exact visible tooltip reason. It then sends physical pointer and focused Enter events while busy; the menu stays closed and selected ID is unchanged. The test records selected ID before each busy/platform transition and verifies it afterward. Quest transition uses an actual switch click from the open-menu state; return to PC also asserts the displayed selected name is retained.
- Final verification on this working tree: Play split tests 15 passed; hero split tests 3 passed; `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo build --release` exited 0; `cargo fmt --all -- --check` and `git diff --check` exited 0. Full `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test` exited 101 with 182 passed, 1 failed, 13 ignored. The only failure remains the sandbox-blocked loopback bind in `core::remote_zip::tests::repairs_single_files_from_a_remote_zip` (`PermissionDenied` at `src/core/remote_zip.rs:187`). Against the pinned baseline (165 passed, 13 ignored), this tree has 17 additional passing tests and the same environment-limited failure.

## Geometry and render fixture notes

- `click_version_choice_at_index` names the popup's test-space anchor y (103.2), popup gap (6), top inset (4), row height (44), and row center (22); the comment ties the anchor to design y=154.8 at the harness's 2/3 scale.
- The progress-width test keeps independent expected rectangles as `Dr::new(139, 88, 160.999, 66.8)` for the PC main region and `Dr::new(139, 88, 305.999, 66.8)` for Quest. The inline comment maps these to approved compact Play design origin x=139/y=88, PC edge x=300, Quest edge x=445, and height 66.8; these values do not derive from production constants.
- Snapshot fractions are named `SNAPSHOT_PROGRESS_FULL` (complete-fill boundary render) and `SNAPSHOT_PROGRESS_42_PERCENT` (partial-fill width render). The ready/installing Quest device test uses a named full-progress fixture constant; progress-width test uses its own named full fraction.
- Re-render command template for either named shot (run once per listed size/output directory):

  `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target ECHOVR_SNAPSHOTS=<output-dir> ECHOVR_SNAPSHOTS_ONLY=<shot-name> ECHOVR_SNAPSHOTS_SIZE=<width>x<height> ECHOVR_SNAPSHOTS_DEMO=1 cargo test --bin EchoVR_Installer headless_snapshots -- --ignored --nocapture`

  For 42%, use shot `launcher_play_quest_installing_42`; outputs are `/tmp/s0-7-luna-finalfix-42-{960,1280,1680}/launcher_play_quest_installing_42.png`. For 100%, use shot `launcher_play_quest_installing`; outputs are `/tmp/s0-7-luna-finalfix-{960,1280,1680}/launcher_play_quest_installing.png`. The independently rendered 42% artifacts from Sol's detached 11a24e8 tree are also at `/tmp/s0-7-sol-b1-42-{960,1280,1680}/launcher_play_quest_installing.png`.
- After naming the geometry/fraction fixtures, all six headless renders were regenerated: the 42% shot passed once each at 960x540, 1280x720, and 1680x720; the 100% shot passed once each at the same sizes. Outputs are the `s0-7-luna-finalfix` paths above.
- Final named-constant verification: Play split tests 15 passed, hero split tests 3 passed, release build passed, fmt/diff checks passed. Full suite remained 182 passed, the same loopback-bind failure, 13 ignored. The independent expected geometry assertions continue to use literal `Dr::new` values rather than production geometry constants.
- The indeterminate fill names `INDETERMINATE_BAND_WIDTH_FRACTION` (0.35) and `INDETERMINATE_CYCLES_PER_SECOND` (0.6), with comments describing their width and motion. Its test phase is the named 0.99 near-end-of-sweep sample. The 35% progress display fixture is also named separately.
- The standalone physical-pointer test uses the accessible arrow node's `rect().center()`. The BAC-0003 test seeds A as the initial selection, chooses B through the menu, and verifies B's preflight path. Each BAC-0004 busy state uses the helper that checks disabled metadata/reason and physical pointer plus Enter rejection. No temporary `eprintln!` remains in `hero.rs`.
- The same named band-width and cycle-rate constants are shared with the original full-width job row, keeping its indeterminate motion aligned with the compact split control. The progress regression test keeps independent approved-design expected rectangles and a named 0.99 sample phase.
- Rechecked after these clarifications: Play split tests 15 passed; hero split tests 3 passed; release build passed; `cargo fmt --all -- --check` and `git diff --check` passed. Full suite: 182 passed, 1 sandbox loopback bind failure at `src/core/remote_zip.rs:187`, 13 ignored.

## Latest-tree verification

- On committed code tree `207f098cdf417a4cdddb46df72b85541b9bf8ad2`, `ui::launcher::play::split_info_tests` passed (15 tests). `cargo fmt --all -- --check` and `git diff --check` passed.
- Full `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test` on that tree: 182 passed, 1 failed, 13 ignored. The failure is the known sandbox-denied loopback bind in `core::remote_zip::tests::repairs_single_files_from_a_remote_zip` at `src/core/remote_zip.rs:187` (`PermissionDenied: Operation not permitted`). The code change since the release build is test-only; the release build passed on the immediately preceding tree.

## BAC-0002 pointer-open red/green recheck

- Reproduced the reported failure against the exact `11a24e8` `widgets.rs` behavior: `toggle_menu` only toggled the open flag, and `menu_popup` treated any click outside both the popup and anchor as dismissal. Temporarily restored those two functions from `git show 11a24e8:src/ui/widgets.rs` while retaining the physical pointer test; the same test failed because the popup was no longer open after the arrow click.
- Red command: `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test --bin EchoVR_Installer ui::launcher::play::split_info_tests::physical_pointer_arrow_opens_only_the_menu -- --nocapture` — failed (exit 101), assertion at `src/ui/launcher/play.rs:1336`, 0 passed / 1 failed / 195 filtered.
- Restored the open-frame guard. `toggle_menu` records the cumulative frame when opening; `menu_popup` skips outside-click dismissal during that frame. Green command: the same focused command — passed (1 passed / 195 filtered).
- The pointer test uses separate move, press, and release passes at the center of the accessible arrow rect. It asserts the popup remains open after release, then asserts a subsequent outside click closes it. This tests the reported click-away race and preserves later outside-click dismissal.

## Astra BAC-0002 follow-up recheck

- Added `pointer_keyboard_and_accesskit_open_ignore_only_the_opener_click`. It performs a real arrow pointer click using the accessible arrow center and separate move/press/release passes; then it clicks the accessible blue `Check for updates` region and confirms the menu closes. This confirms the update region is not part of the opener exemption.
- The same regression independently opens the menu with a physical Enter key event and with AccessKit activation. A subsequent outside pointer click closes the menu in both cases.
- Reproduced red against the pinned `11a24e8` production dismissal behavior by temporarily restoring its `toggle_menu` and unconditional outside-click check in `widgets.rs`. The expanded test failed (exit 101) at `src/ui/launcher/play.rs:1364` because the menu-open flag was false after the physical arrow click. Restored the current cumulative-frame opener guard; the test passed (1 passed / 196 filtered). This guard leaves the popup anchor unchanged and only skips outside-click dismissal on the cumulative pass where the menu is opened.
- Final focused verification: `ui::launcher::play::split_info_tests` — 16 passed, including installed-B menu routing and BAC-0004 busy reason/selection/activation assertions; `ui::launcher::hero::play_split_tests` — 3 passed. `cargo fmt --all -- --check` and `git diff --check` passed; release build passed. Full `cargo test` had 183 passed, 1 failed, 13 ignored; the sole failure remains the sandbox loopback bind permission error at `src/core/remote_zip.rs:187`.

## Astra follow-up: opener-only dismissal exclusion

- Replaced the cumulative-frame exemption with an explicit `dismissal_exclusion` rectangle in `menu_popup`. `version_picker` passes `hero::compact_play_arrow_region()` converted to screen coordinates, matching the actual arrow hit/accessibility rectangle. The 404×1 positioning anchor at design y=154.8 is unchanged.
- Outside-click dismissal now excludes only clicks inside the popup, its original thin positioning anchor, or the arrow opener. The blue Update region is outside all three and remains dismissible. There is no cumulative-pass marker, so additional egui passes in the opening frame cannot exempt unrelated click positions.
- `pointer_keyboard_and_accesskit_open_ignore_only_the_opener_click` derives its physical opener point from `get_by_label("Choose PC version").rect().center()`. It asserts the release opens the menu, then clicks the actual accessible `Check for updates` rect center and asserts dismissal. It separately verifies keyboard and AccessKit open paths followed by later outside-click dismissal. The independent `physical_pointer_arrow_opens_only_the_menu` also verifies no page/action/selection change on open and later generic outside dismissal.
- Red/green: temporarily removed the `dismissal_exclusion` containment check while retaining the exact arrow region parameter. The focused regression failed (exit 101) at the open-state assertion (`src/ui/launcher/play.rs:1365`), reproducing the 11a24e8 unconditional outside-click behavior. Restoring the opener-only check passed the test. The complete Play split suite passed 16 tests; hero split tests passed 3; release build passed. Final full-suite result is recorded below after completion.
- Final full-suite run after the opener-only geometry fix: 183 passed, 1 failed, 13 ignored. The sole failure is the known sandbox-blocked loopback bind in `core::remote_zip::tests::repairs_single_files_from_a_remote_zip` (`PermissionDenied` at `src/core/remote_zip.rs:187`). `cargo fmt --all -- --check` and `git diff --check` pass on the final diff.

## Final acceptance recheck

- Confirmed `compact_job_progress_body(has_arrow)` uses `compact_play_regions(has_arrow).0`; it selects the full Quest main bounds when there is no arrow. There is no `let _ = has_arrow` red mutation in the worktree.
- Confirmed the real arrow pointer tests use `get_by_label("Choose PC version").rect().center()`; no `(235, 80)` coordinate exists in the current Play tests. Updated the installed-B launch test to click the accessible PLAY rect center as well, and corrected the pointer test comment to describe the arrow-only exclusion rather than the superseded frame guard.
- Rechecked after cleanup: Play split tests 16 passed (including BAC-0003 installed B chosen through the menu and BAC-0004 busy reason/selection/pointer+keyboard attempts); hero split tests 3 passed (including Quest full-width and PC-capped B1 assertions). Formatting and diff checks pass. The geometry-based implementation and blue Update dismissal regression remain in place.

## User diff review follow-up

- Re-inspected the current tree: there is no `opened-frame` guard or B1 red mutation. Progress body uses `compact_play_regions(has_arrow).0`; physical arrow clicks derive their point from the accessible `Choose PC version` rect, then assert the popup remains open after release. The next click uses the accessible blue Update rect and asserts the popup closes.
- BAC-0003 selects installed B through the popup before activating PLAY; the main click now also uses the accessible PLAY rect. BAC-0004 retains exact disabled reason, unchanged selected ID, and disabled pointer/keyboard activation assertions for each required busy state and platform switch.
- Added a rationale for the named indeterminate progress band and sweep rate. Final focused rerun: Play split tests 16 passed; hero split tests 3 passed, including Quest full-width and PC-bounded progress. `cargo fmt --all -- --check` and `git diff --check` passed.

## BAC-0003/0004 source-level clarification

- Clarified the BAC-0003 fixture name to `installed_b`; the test explicitly starts with `other-a` selected, opens the arrow menu, chooses `selected-b` through the installed-choice row helper, asserts the menu closed and selected ID changed to B, then activates PLAY and verifies the missing-executable preflight references B's distinct root rather than A's.
- Reconfirmed `assert_busy_arrow_rejected` checks disabled AccessKit metadata, hovers the disabled arrow to assert the exact reason, sends a real pointer click and focused Enter while disabled, and after each attempt asserts the selected ID is unchanged and the menu remains closed. The transition test invokes it for launch-start, external-running, owned-running, selected install, and unrelated job; it also asserts selected ID after each transition.
- Focused rerun after these assertions: Play split tests 16 passed; `cargo fmt --all -- --check` and `git diff --check` passed.

## Physical arrow opener and blue Update dismissal assertion

- Renamed the regression to `physical_pointer_arrow_opens_then_blue_update_click_closes_menu` and made it finish with a physical click at `get_by_label("Check for updates").rect().center()`. It first asserts the pointer release on the dynamic `Choose PC version` rect center leaves the popup open and does not change the main action, page, selection, jobs, update note, or dialog state. The later blue Update click closes the menu while leaving page and selected version intact; the click itself is allowed to start the update-check action.
- The physical pointer dismissal path clicks the actual accessible blue Check for updates rect center. Keyboard and AccessKit paths use the named `OUTSIDE_MENU_TEST_POINT`, documented as blank backdrop outside the popup and anchor at the 1280×720 harness size. The physical pointer test verifies its opening click leaves the primary action unchanged, then asserts the later blue click changes the primary action to Update (`Updating` and `Checking for updates…`) while closing the menu. Indeterminate width/speed constants and the named 0.99 test phase have inline rationale comments.
- The focused Play split suite passed 16 tests after this assertion change; fmt and diff checks passed.

- Final blue-region verification: `physical_pointer_arrow_opens_then_blue_update_click_closes_menu` passed after asserting Update dispatch and menu closure; the `OUTSIDE_MENU_TEST_POINT` fixture is named and documented. `cargo fmt --all -- --check` and `git diff --check` passed.

- Final coordinate cleanup: keyboard and AccessKit outside-dismiss checks now use the named blank-backdrop point; the physical arrow test retains the actual blue Update rect click and Update-dispatch assertion.

## Installed target action coordinates

- In `missing_installed_target_routes_only_its_catalogue_id_to_install`, both main-action clicks now use the accessible `PLAY` widget rect center rather than fixed screen coordinates. The test still verifies that a missing installed target with a catalogue ID routes that ID to Install and one without an ID routes `None`.
- Verification after this coordinate cleanup: `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test --bin EchoVR_Installer ui::launcher::play::split_info_tests -- --nocapture` — 16 passed; `cargo fmt --all -- --check` and `git diff --check` passed.

## Arrow dead-corner dismissal regression

- Added `inert_upper_right_arrow_corner_dismisses_open_menu`. It opens through a physical click at the accessible arrow center, computes Astra's reported upper-right dead-corner point from that widget rect using named 10 px / 1 px inset constants, verifies the point is inside the arrow rect, clicks it as a later event, and asserts the menu closes without changing page, selection, or main action. The existing physical blue Check for updates dismissal/dispatch test remains unchanged.
- Red reproduction on the rectangle-only implementation at `41965c6`: `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo test --bin EchoVR_Installer ui::launcher::play::split_info_tests::inert_upper_right_arrow_corner_dismisses_open_menu -- --nocapture` failed at the menu-closed assertion; the inert corner left the menu open.
- Green fix: the exclusion now requires the point to be inside both the arrow bounds and the same compact Play polygon used by `hot_shape`; compact row drawing and hit ownership share that polygon helper. The original thin popup positioning anchor is unchanged, and the later blue Update click remains outside the exclusion.
- Focused verification after the fix: Play split tests 17 passed; hero split tests 3 passed; `cargo fmt --all -- --check`, `git diff --check`, and `CARGO_TARGET_DIR=/home/andrew/src/evr-launcher/.cargo-target cargo build --release` passed.

## PR #4 CI and three-model review follow-up

- GitHub Actions at the original clean PR head `16cd62b` passed Ubuntu and macOS. Windows exposed one platform-sensitive test fixture: `installed_main_routes_the_selected_version_without_launching_a_different_one` inherited Meta Link as the launch runtime, so Windows correctly showed its live Oculus-runtime confirmation before reaching the missing-executable assertion. The fixture now selects Flat explicitly; this keeps the route-to-selected-B assertion deterministic and does not change production runtime behavior.
- Independent review of the same PR head found the Quest ready/not-installed status duplicated between the primary and detail lines; during installation, the stale detail could remain under progress. `quest_action` now removes that repeated detail while retaining the device name. The Quest test asserts the exact primary and secondary status for ready/not-installed and installing states, plus one device occurrence across both lines.
- Added `physical_pointer_ownership_matches_clipped_polygons_at_all_design_sizes`, driving real pointer move/press/release events at design samples on both sides of x=300 and x=445, the seam strokes, both slanted corners, and main/arrow/Update interiors at 960×540, 1280×720, and 1680×720. The inert arrow corner dismissal is also exercised at each viewport scale. Expected samples are named independently of production geometry constants.
- Latest local checks on the PR follow-up tree: Play split tests 18 passed; hero split tests 3 passed; Clippy `-D warnings` passed; release build passed; formatting and `git diff --check` passed; full `cargo test` passed with 186 passed, 0 failed, 13 ignored. The earlier loopback-bind failure no longer reproduces in this environment.
- The fixes above are not yet on GitHub at the time this note was written. Push them to PR #4, then wait for the Windows workflow to rerun before recording its final status.
