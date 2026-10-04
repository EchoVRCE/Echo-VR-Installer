# S0-7 Play split: Astra design review

Verdict: **changes requested; implementation handoff blocked**. Two blocking findings and four non-blocking findings below. This is a design review, not approval of an implementation or PR for merge.

Reviewed design commit: `592254ece3c919f82fedc3a6a712039aff8164d5` on `sprint/s0-7-play-split`. Code baseline: `0a85ed94413a44fb1e087acd21e24029b5abe1f4`. Reviewer: Astra. Review date: 2026-10-03 (CST6CDT).

## Scope and source audit

Read the courier instructions, parent and worktree `AGENTS.md` in full, global `CLAUDE.md`, and the `start-sprint` and `review-design` skills. Read the complete [design](s0-7-play-split.md), all four linked BACs, and every cited source line personally. There is no root-worktree `AGENTS.md`; the applicable repository rules are in this assigned worktree.

The following are the design's complete 13 source references, checked against the pinned base. Every referenced file is byte-identical between that base and the reviewed design commit:

| Source read | Claim assessed |
| --- | --- |
| `src/ui/launcher/play.rs:72` | Logo draw |
| `src/ui/launcher/play.rs:85-99` | Info, normal/job row, switch, picker and news order |
| `src/ui/launcher/hero.rs:22-75` | Existing row geometry, slants and switch |
| `src/ui/launcher/play.rs:31-42` | News/card boundaries |
| `src/ui/launcher/hero.rs:285-320` | Main hit target, accessible name and click result |
| `src/ui/launcher/play.rs:480-528` | Main and update action dispatch |
| `src/ui/launcher/hero.rs:548-589` | Platform switch behavior |
| `src/ui/launcher/play.rs:666-815` | Entries, selected name, busy gates, persistence and Install route |
| `src/ui/launcher/mod.rs:990-993` | Snapshot menu key |
| `src/core/launcher/store.rs:256-290` | Target resolution and fallback |
| `src/ui/launcher/play.rs:497-516` | Selected catalogue/missing Install dispatch |
| `src/ui/launcher/play.rs:1050-1075` | Start/preflight entry, including event-build branch |
| `src/ui/launcher/install.rs:75-105` | Shared hero offset and normal/job controls |

Additional reads covered PC and Quest action construction, the hidden logo interaction, info-line rendering, the remainder of preflight and the start target lookup, menu lifecycle, hit-shape handling, persistence errors, native window sizing/scaling, and the headless screenshot harness. The observations below cite these sources individually.

The visual reference is **unexamined**: `docs/design/s0-7-play-split.md:3` names `mockup-v1.png` without a resolvable path. A filename search across this repository (including worktrees and `.orient`) and `/tmp` found no matching mockup. Glow was asked by courier for its location. No assertion of visual fidelity or of Andrew's unstated Quest preference is made here.

## Findings

### B1 — blocking: removing VERSION loses the selected name for catalogue and missing targets

**Evidence:** `docs/design/s0-7-play-split.md:7` promises that the selected version remains visible in the status line; `docs/bac/BAC-0002-play-version-arrow.md:5` expects selection to update visible status. The present picker obtains both installed/missing and catalogue names at `src/ui/launcher/play.rs:684-687` and paints the name at `src/ui/launcher/play.rs:736-745`. The retained info line only adds a version name for `Target::Installed` at `src/ui/launcher/play.rs:282-283`. The missing and available branches discard their target values at `src/ui/launcher/play.rs:359-368`; `Action::not_installed` produces only a status or install progress at `src/ui/launcher/play.rs:167-179`.

**Impact:** Reusing the status construction while deleting the picker makes two different uninstalled selections indistinguishable after the menu closes. A user cannot see which version the next Install action targets. This contradicts the stated visible-selection contract, even if the saved ID and launch routing remain correct.

**Required resolution:** Explicitly include the selected name in the PC status contract for installed, missing and catalogue targets, including their applicable progress states. Preserve the separate empty-state message for `Target::None`. Add these fixtures to BAC-0001/0002; do not treat the checked item in a closed menu as sufficient visibility.

**Recheck:** Select catalogue A, then catalogue B, closing the menu each time; assert their distinct names remain visible with “Not installed.” Repeat with a named missing installed entry and with an installation in progress. Verify that the no-target case does not invent a selection. A deliberately unchanged `not_installed` line must fail this check.

### B2 — blocking: the geometry review is deferred without a reviewable split layout

**Evidence:** `docs/design/s0-7-play-split.md:23` leaves hit geometry, supported sizes and selected-name visibility for this review, and line 32 requires resolution before implementation. No proposed main/arrow boundary, arrow width, row bounds or label-space policy is supplied at lines 11-19. `docs/bac/BAC-0001-play-row-layout.md:5-7` checks only an unnamed default size. The actual default is 1280×720 (`src/ui/launcher/mod.rs:50-51`), and the native minimum is 960×540 (`src/ui/mod.rs:27-28`); resizing also changes zoom (`src/ui/mod.rs:112-120`). Existing Play and Update use separate interaction rectangles and polygons (`src/ui/launcher/hero.rs:29-36`, `src/ui/launcher/hero.rs:57-64`, `src/ui/design.rs:381-413`). Simply adding a rectangular arrow over the old main target can leave overlapping ownership or active empty corners. The cited mockup is unavailable and, by the design's own account, depicts a separate picker rather than the proposed split (`docs/design/s0-7-play-split.md:3`).

**Impact:** The review cannot establish that the proposed arrow fits the slanted boundary, preserves useful main-label space, and remains usable at the supported minimum. Those are explicitly unresolved design decisions, not evidence of a demonstrated rendering bug. A screenshot at default size alone cannot discriminate incorrect hit ownership.

**Required resolution:** Supply a resolvable visual reference and a proposed annotated layout or coordinate constraints for the split, its menu anchor, labels and row/info bounds. Specify exclusive main/arrow/update hit ownership, including polygon corners and the boundary. A Play-only widened green button with a reserved arrow segment is a viable option; carving an arrow out of the existing narrow label area needs evidence that PLAY/PATCH/SET UP/STOP and progress remain readable. State the choice in the design and extend BAC-0001/0002 to default, minimum and a wide viewport. Keep Install's geometry opt-in as already required.

**Recheck:** Review the supplied layout at 1280×720, 960×540 and 1680×720 in both platforms and job states. At each size, sample each region's interior and both sides of the main/arrow and arrow/update boundaries, plus slant corners: exactly the intended action fires; empty corners fire none. Check the popup anchor and readable label/status bounds. Compare Install normal and job layouts against the baseline. Resolve this finding in a revised design before handing geometry choices to implementation.

### N1 — non-blocking: account for the invisible interaction left behind by the logo

**Evidence:** The layout seam cites the logo at `src/ui/launcher/play.rs:72` but omits the immediately following `easter_egg` call at line 73. That function registers an invisible click rectangle `(574, 74.4, 50, 126.5)` in design coordinates and opens a dialog (`src/ui/launcher/play.rs:220-235`). This is inside the old logo band where the new row is intended to go (`docs/design/s0-7-play-split.md:7`).

**Impact:** Deleting only the logo image leaves an unexplained active area; depending on the chosen layout, it may overlap an action or occupy blank space. The exact new overlap cannot be claimed before B2 is resolved.

**Recommendation/recheck:** Retire this Play-logo hit target along with the logo, or explicitly choose a non-overlapping destination. Click the old hotspot across its extent after relocation; it must not open the Easter Egg dialog unexpectedly, and any new visible control there must receive its own action exactly once. Add this to BAC-0001's interactive-region inspection.

### N2 — non-blocking: include accessibility and zero/one-version fixtures

**Evidence:** Distinct accessible names are required by `docs/design/s0-7-play-split.md:12`, but `docs/bac/BAC-0002-play-version-arrow.md:5-7` tests clicks with at least two selectable versions only. The empty-list rule at design line 19 does not choose hidden versus disabled presentation. Existing empty lists return before drawing the picker (`src/ui/launcher/play.rs:681-683`); a one-entry list still includes “Install another version” (`src/ui/launcher/play.rs:796-800`). `hot_shape` supports pointer-free activation and supplies button metadata (`src/ui/design.rs:394-413`).

**Impact:** Pointer screenshots could pass while the arrow lacks a separate accessible name/activation, or while an implementation wrongly suppresses the useful one-entry menu.

**Recommendation/recheck:** Add two distinctly named/focusable controls and separate keyboard/accessibility activation checks. Include zero entries (no empty popup; choose its presentation explicitly) and one entry (current checkmark plus Install another version). Verify the arrow never dispatches main/update actions. A shared accessible target or `entries.len() < 2` suppression must fail.

### N3 — non-blocking: specify the missing-files oracle rather than only a target ID

**Evidence:** `docs/bac/BAC-0003-play-selected-version.md:5-7` preserves “existing missing-files behavior” and proposes asserting a target ID for every fixture. The current missing target takes `Main::ToInstall` (`src/ui/launcher/play.rs:167-170`, `src/ui/launcher/play.rs:359-365`), which sets `install_pick` to the missing entry's optional **catalogue ID**, not its installed ID (`src/ui/launcher/play.rs:509-516`). An external entry can have no catalogue ID. The separate executable-missing error in `start` is a later guard (`src/ui/launcher/play.rs:1115-1130`).

**Impact:** An implementation test could demand the wrong ID or confuse an already-missing target with files disappearing after action selection.

**Recommendation/recheck:** Specify two missing-entry fixtures: with `catalog_id`, expect `Page::Install` and that catalogue ID; without one, expect `Page::Install` and `install_pick == None`. Neither spawns a game. Keep the later missing-executable error as an existing launch guard. Reject a test that asserts the installed ID is always passed to Install.

### N4 — non-blocking: busy coverage should include transitions and the starting interval

**Evidence:** `docs/bac/BAC-0004-play-platform-and-busy.md:5-7` names running and active-job fixtures. The current gate also covers `d.ours()` before the monitor reports a running game (`src/ui/launcher/play.rs:689-698`), while normal versus job rendering takes separate paths (`src/ui/launcher/play.rs:87-97`). Menu-open state is stored independently (`src/ui/widgets.rs:707-717`); disabling the old picker skips rendering the popup (`src/ui/launcher/play.rs:757-759`) rather than clearing its stored flag.

**Impact:** Only entering the page already busy does not test an open menu becoming busy, the launch-start interval, or the job-row replacement. Reusing the main action's enabled flag would also be wrong: an owned running game allows STOP (`src/ui/launcher/play.rs:296-304`) while version switching is forbidden.

**Recommendation/recheck:** Open the menu while idle, then enter starting/running, a selected-target job, and an unrelated job; assert no displayed/selectable version menu or saved selection change and a reason on the disabled arrow. Verify STOP and applicable progress/CANCEL independently. Switch an open-menu PC view to Quest and back; explicitly choose whether the old menu state is cleared or restored. The preserved PC selection must not depend on that choice.

## BAC-by-BAC discrimination and trace

All four BACs link to the governing design and all four design links resolve. The matrix describes proposed checks, **not tests executed for new behavior**.

| BAC | Design trace and concrete setup/action/oracle | Plausible wrong implementation caught | Assessment |
| --- | --- | --- | --- |
| [0001](../bac/BAC-0001-play-row-layout.md) | Design lines 7, 11, 15. Render Play PC/Quest and Install normal/job fixtures; compare row bounds, logo/picker absence, readable status and non-overlap. | Hides VERSION while leaving an empty logo band, or moves Install via shared constants. | Observable but blocked by B1/B2; add N1 and the named sizes. |
| [0002](../bac/BAC-0002-play-version-arrow.md) | Design lines 12-13, 19. Use two installed IDs plus an uninstalled catalogue entry, select through arrow, reopen to check the mark, then reload saved state from an isolated writable location; exercise Install another version and the snapshot key. | Arrow also launches; choice only changes an in-memory label; catalogue duplicates an installed entry; popup stays at the old picker. | Strong core oracle; B1/B2 and N2 need incorporation. Demo screenshots alone cannot prove persistence: `Dashboard::save` returns in demo mode (`src/ui/launcher/mod.rs:560-565`). |
| [0003](../bac/BAC-0003-play-selected-version.md) | Design line 14. Give A and B distinct IDs/paths, satisfy prerequisites, select B and capture its existing launch/preflight route; choose catalogue C and check Install selection; exercise both missing-entry cases in N3. | Launches default A despite choosing B, bypasses preflight, or starts/install-dispatches on opening the menu. | Testable; use route/selected target observation without launching a real game. Clarify N3's oracle. |
| [0004](../bac/BAC-0004-play-platform-and-busy.md) | Design lines 12, 19. Owned-running, externally-running, active-job and Quest fixtures; attempt arrow activation, check reason and independent main/update actions, then return to PC. | Disables STOP with the arrow, leaves the picker active during a job, shows PC entries on Quest, or resets PC choice on return. | Testable; expand with N4. Quest omission is internally consistent with the current PC-only picker (`src/ui/launcher/play.rs:95-98`), but does not establish unstated maintainer preference. |

## Design boundaries and alternatives

Reusing menu construction, `LauncherState.selected`, target resolution and the current main dispatch is justified by the cited source. A second selection model would add synchronization and migration work without serving the stated scope. A separate Play renderer or an explicit opt-in shared split renderer both satisfy the Install boundary; changing shared geometry unconditionally does not (`src/ui/launcher/install.rs:79-105`). Include `job_row`, not just `row`, in whichever opt-in strategy is selected.

This design adds no download, process privilege or catalogue-trust boundary; it should continue through the existing launch/Install/preflight paths. Persistence remains the current atomic temporary-file/rename write (`src/core/launcher/store.rs:243-248`); save failures are logged (`src/ui/launcher/mod.rs:564-565`). Do not promise restart persistence when saving fails. No stored-state migration is needed if the selected-ID contract is retained. A UI rollback should restore the old picker while preserving that ID, so no library or installed-version deletion is needed. These are scope assessments from the proposed reuse, not a security audit of those subsystems.

## Verification and handoff

- Source audit: all 13 cited ranges exist and their complete files match the pinned base; all design/BAC links resolve. A Python check using `git show <base>:<path>` and range/link assertions exited 0 with `13 source references audited; design/four BAC links resolve`. `git diff 0a85ed94413a44fb1e087acd21e24029b5abe1f4 HEAD -- src` exited 0 with no output.
- Baseline commands ran from `.worktrees/s0-7` at `592254ece3c919f82fedc3a6a712039aff8164d5`, with only this review document untracked. `cargo build --release` exited 0: ``Finished `release` profile [optimized] target(s) in 1m 35s``. `cargo test` reported `164 passed; 1 failed; 13 ignored`; a repeat to confirm the failing run's exit status exited 101 with `test result: FAILED. 164 passed; 1 failed; 13 ignored; 0 measured; 0 filtered out; finished in 1.32s`. The single failure is `core::remote_zip::tests::repairs_single_files_from_a_remote_zip`: `src/core/remote_zip.rs:187` binds `127.0.0.1:0` and receives `Os { code: 1, kind: PermissionDenied, message: "Operation not permitted" }`. This accounts for the one-pass difference from the recorded `ad9f709` baseline of 165 passed/13 ignored; the test cannot run its local server in this environment. The full test gate is **not passed**. Logs: main worktree `.orient/s0-7-astra-build.log` and `.orient/s0-7-astra-test.log` (the latter contains the confirming run).
- No implementation, new tests, mutation tests, screenshot renders or live game/Quest checks have been performed for S0-7. No new BAC is claimed satisfied by unchanged-code tests. The source audit and document diff check validate this review artifact; the proposed behavioral rechecks above remain implementation work.
- Hygiene covered all four registered worktrees (`main`, `adr/0001-dll-slot`, `sprint/s0-7-play-split`, `stage0/roles`), every local branch/upstream, cached remote branches and the stash namespace. All worktrees were initially clean, none detached, and no stashes existed. Three branches matched their cached upstreams; `adr/0001-dll-slot` has none. Commit subjects ahead of `origin/launcher-m1` were inspected and preserved (five main-branch commits; six workflow commits on ADR/roles; those six plus this design on S0-7). Remote freshness is unverified: `git ls-remote --exit-code origin HEAD` exited 128 with `Bad owner or permissions on /etc/ssh/ssh_config.d/20-systemd-ssh-proxy.conf`.
- B1/B2 remain open; no revised design has been reviewed. Sol should revise the design/BACs and request Astra recheck before Luna implementation, per `AGENTS.md`'s Handoffs section. Glow receives this artifact path and review commit for routing to Sol and pushing; Astra does not push.
