# S0-7: Play split button and compact action row

Status: Proposed for design review. Base code: `0a85ed94413a44fb1e087acd21e24029b5abe1f4`. Visual input: Andrew's `mockup-v1.png` in the supplied scratchpad; it shows the desired top-row placement but still has a separate VERSION control. The requested split button supersedes that control.

## Intent

Put Play, Check for Updates, and the PCVR/Quest toggle at the top of the Play panel, in the space now occupied by the large Echo VR logo. Make the green Play control a split button: its main region performs the selected version's action, and a separate down-arrow region opens the version list. Remove the standalone VERSION dropdown. The selected version remains visible in the status/info line and as the checked menu item.

## Current code and proposed seam

- The Play page draws the logo at `src/ui/launcher/play.rs:72`, then the info line, action buttons, platform switch, and separate PC version picker at `src/ui/launcher/play.rs:85-99` (@ `0a85ed9`). Its current coordinates put the logo at the top and the buttons below it: `src/ui/launcher/hero.rs:22-75` (@ `0a85ed9`). Community News begins below the row at `src/ui/launcher/play.rs:31-42` (@ `0a85ed9`). Move only the Play page's row layout into the logo area and retain a readable info line before the news cards.
- `hero::row` currently gives the entire green shape one hit target and returns one main click (`src/ui/launcher/hero.rs:285-320` @ `0a85ed9`). The proposed split needs distinct hit targets and accessible names for main and arrow; the arrow must not invoke the main action. The update control and platform switch keep their existing actions (`src/ui/launcher/play.rs:480-528`, `src/ui/launcher/hero.rs:548-589` @ `0a85ed9`).
- The existing PC picker already builds installed and catalogue entries, blocks switching while the game runs or a job is active, persists a choice, and routes the final entry to Install (`src/ui/launcher/play.rs:666-815` @ `0a85ed9`). Reuse this menu content and selection behavior under the arrow; remove its separate VERSION rectangle and caption. `SnapVariant::VersionMenu` opens that menu by key (`src/ui/launcher/mod.rs:990-993` @ `0a85ed9`), so keep the snapshot route working when the anchor moves.
- A selected ID is resolved to an installed, missing, or catalogue target by `src/core/launcher/store.rs:256-290` (@ `0a85ed9`). Main Play already calls `try_start` for that target (`src/ui/launcher/play.rs:497-516`, `src/ui/launcher/play.rs:1050-1075` @ `0a85ed9`). Preserve these launch and Install paths; this is a control/layout change, not a new version-selection model.
- Install also uses `hero::row` and `hero::switch` with its own offset (`src/ui/launcher/install.rs:75-105` @ `0a85ed9`). Scope split-button geometry and compact placement to Play, or make shared changes opt-in, so Install's controls do not move or acquire an unintended version arrow.

## Proposed interaction

On PCVR, a distinct down arrow at Play's right edge opens the current version list. Choosing an item updates the selected version and closes the menu; a click on the rest of Play continues to act on that selection. While switching is unsafe because the game is running or a job is active, the arrow is disabled and explains why, matching the current picker. If no PC versions are known, the arrow has no empty menu to open. On Quest, retain its present main action and omit the PC version arrow because the list is PC-only. Keep the separate Check for Updates and platform controls.

## Review points

Astra should check the split's hit geometry against the slanted Play/Update boundary, the row's fit at supported window sizes, whether the selected version is visible enough without VERSION, and whether hiding the arrow on Quest matches Andrew's intent. The reference mockup does not specify those edge cases.

## Acceptance criteria

- [BAC-0001](../bac/BAC-0001-play-row-layout.md): compact Play row and removal of the standalone picker.
- [BAC-0002](../bac/BAC-0002-play-version-arrow.md): separate arrow, version list, and selection.
- [BAC-0003](../bac/BAC-0003-play-selected-version.md): main Play uses the selected target.
- [BAC-0004](../bac/BAC-0004-play-platform-and-busy.md): Quest and busy-state behavior.

Implementation and code tests are outside this design-only sprint item. Design review must resolve the review points before implementation.
