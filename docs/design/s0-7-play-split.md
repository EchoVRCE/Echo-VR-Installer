# S0-7: Play split button and compact action row

Status: Revised proposal for Astra recheck. Code baseline: 0a85ed94413a44fb1e087acd21e24029b5abe1f4. This revision answers [Astra's review](s0-7-play-split.review.md) at 72d2195.

Visual references: [requested compact row](refs/s0-7-mockup-v1.png) and [original layout](refs/s0-7-original.png). The requested image moves the row into the logo band but retains a separate VERSION picker. The split arrow and geometry below replace that picker. These images are references, not a rendering claim for unimplemented behavior.

## Intent and scope

Put Play, Check for Updates, and the PCVR/Quest toggle at the top of the Play panel, in the space now occupied by the large Echo VR logo. On PC, make the green action a split control: its main region acts on the selected version and its arrow opens the existing version list. Remove the separate VERSION rectangle and caption. Keep the selected PC version's name visible after the list closes, even when it is a catalogue entry or its installed files are missing. Quest keeps the main action without a PC version arrow.

The Play page alone opts into the compact/split row. Its normal and job paths both use that layout. Install retains its existing hero::row, hero::job_row, hero::switch, offset and hit geometry (src/ui/launcher/install.rs:75-105 @ baseline). Selection remains LauncherState.selected; target resolution and launch/Install/preflight routing stay as they are (src/core/launcher/store.rs:256-290, src/ui/launcher/play.rs:497-516, src/ui/launcher/play.rs:1050-1075 @ baseline).

## Layout and hit contract

Coordinates below are **1920×1080 design pixels** before dz(2/3) and window zoom. The native default is 1280×720 and minimum is 960×540 (src/ui/launcher/mod.rs:50-51, src/ui/mod.rs:23-29 @ baseline). fit_zoom scales the design uniformly; additional width extends the content column rather than stretching the row (src/ui/mod.rs:110-121 @ baseline). Review at 1280×720, 960×540 and 1680×720; the last has 600 extra design pixels of content width. These are the named viewport fixtures, not a claim that other sizes are unsupported.

| Play-only region | Proposed design bounds / behavior |
| --- | --- |
| Compact row | Green and blue shapes use y=88..154.8, translating the existing row up 155.0. Widen green by 120.0; move blue and the platform switch right by 120.0. The row occupies x=139..964, above the info line. Both normal and job rows use this geometry. |
| PC main | x=139..300, y=88..154.8; the left green rectangle, with a label centered at x=219.5. Its label has at least 130 design pixels of room for PLAY, PATCH, SET UP, STOP or a progress value. Progress fill and percentage are clipped to this main segment. |
| PC arrow | x=300..445, y=88..154.8, clipped to the widened green trapezoid. The visible chevron is centered about (355,121) and a divider marks x=300. At the top the arrow's green width is 89.4 design pixels, equivalent to about 44.7 logical pixels at the native minimum. It keeps a distinct accessible name and focus target. |
| Update / Cancel | Existing blue shape translated right and up: its painted polygon runs from x=440.1..783; its active polygon is clipped to x>=445. Its label center is (620.4,121.8). Update and Cancel retain their independent enabled states and dispatch. |
| Platform switch | Existing switch translated right and up to about x=817..964, y=93.4..153.4, including its captions. Quest has the same row positions, but the full green shape is one main target with no arrow or divider. |
| Info and news | The info line owns x=144..1282+dx, y=171..218. Community News starts at y=230 (120 above its old y=350); its banner, link and cards move up together, with cards beginning at y=680. Server Info stays in its right column. No row, info or news bounds overlap. |
| Version popup | Anchor to the PC arrow's rectangle, below its y=154.8 lower edge with the existing small popup gap, and use 404 design pixels of width from x=300..704, within the left content column at all named sizes. Limit popup height to the remaining viewport and scroll long lists rather than flipping above the top row or leaving the window. The foreground popup may cover the info/news while open; it does not move them. Keep the play_version_menu snapshot key and current entries/checkmark/Install another version behavior (src/ui/launcher/play.rs:666-815, src/ui/launcher/mod.rs:990-993 @ baseline). |

The widened green painted polygon is (139,88), (389.4,88), (452,154.8), (139,154.8); the translated blue polygon is (440.1,88), (783,88), (783,154), (503.2,154) (src/ui/launcher/hero.rs:29-64 @ baseline). Clip **both paint and pointer ownership** at the vertical seam x=445: green to its left, blue to its right. The PC main owns x<300 within green; the arrow owns 300<x<445 within green; Update owns x>445 within blue. The exact divider/seam strokes are inert. This produces disjoint interaction rectangles; no rectangular arrow is layered over the old Play target. At the slanted tips and other corners outside the clipped polygons, no action fires. Pointer-free activation still addresses each separately named control (src/ui/design.rs:381-413 @ baseline). Test immediately to either side of both seams and at the slanted corners.

At the native minimum, the row, info and cards keep these relative design bounds under uniform zoom. The main label remains in its 130-design-pixel slot instead of losing width to the arrow; progress never paints beneath the arrow. At 1680×720, extra width grows the info/news column, while the row and popup anchor remain fixed. If a label cannot fit the main slot at the existing Play font, use the existing fit-to-width behavior without clipping the word (src/ui/launcher/hero.rs:519-545 @ baseline). Compare normal/job Play and normal/job Install at all three sizes before approving implementation.

Retire the Play-logo Easter Egg click rectangle with the image. easter_egg currently registers an invisible (574,74.4,50,126.5) region (src/ui/launcher/play.rs:72-73, src/ui/launcher/play.rs:220-235 @ baseline); it must not survive as an invisible action over or beside the compact row. This does not change Easter Eggs on other pages.

## Selected PC version and menu state

The Play info line must build its **primary group from the resolved target** before optional details. Show the actual selected name for Target::Installed(v), Target::Missing(v) and Target::Available(e) in normal, running, starting and applicable job/progress states. Examples are Echo VR (PC, Latest) · Installed; Echo VR (PC, Latest) · Game files missing; Echo VR (PC, Beta) · Not installed; and Echo VR (PC, Beta) · Installing · 35%. For Target::None, show a separate no-selection message such as No PC version selected · Install a version; do not invent a name. The current code only appends the installed name and drops names in the missing/available branches (src/ui/launcher/play.rs:282-294, src/ui/launcher/play.rs:359-368 @ baseline).

Put the name and state/progress ahead of path, server count and other optional details. Use up to two lines in the y=171..218 info band if needed; elide trailing details first. If an unusually long name alone exceeds that band, show a visible ellipsis and expose its full text by tooltip/accessibility. A normal catalogue or missing-entry name must remain distinguishable with the menu closed, including during installation. Preserve the installed path click when its path is actually displayed (src/ui/launcher/hero.rs:189-255 @ baseline).

On PC with zero installed and zero catalogue entries, hide the arrow and divider; the full green control takes the main action and no empty popup exists. With one entry, show the arrow: the checked entry and Install another version still make a useful menu. Give the main and arrow distinct accessible names and keyboard activation; arrow activation never dispatches Play/Update. On Quest the arrow and PC menu are absent.

The arrow is visible but disabled with a reason during the launch-start interval (d.ours()), an owned or external running game, any launcher job, and a selected-target job. The main action remains independent: STOP can still work for an owned game, and progress/CANCEL remain available for applicable jobs. When an open menu becomes busy or the platform changes to Quest, close and clear its temporary open flag immediately. Returning to idle PC leaves the menu closed while preserving the selected PC ID. Reuse the existing saved-ID behavior and report a save failure rather than promising restart persistence when saving fails (src/ui/launcher/play.rs:689-698, src/ui/widgets.rs:707-723, src/ui/launcher/mod.rs:560-565 @ baseline).

## Review disposition

| Finding | Resolution in this revision |
| --- | --- |
| B1 | The target-specific info-line contract above includes installed, missing and catalogue names, their progress states, and the separate None message. [BAC-0001](../bac/BAC-0001-play-row-layout.md) and [BAC-0002](../bac/BAC-0002-play-version-arrow.md) use named A/B and missing/job fixtures. |
| B2 | The coordinate table, clipped polygons, exclusive seams, popup anchor, label policy and named viewport fixtures above make layout and hit ownership reviewable before implementation. BAC-0001/0002 require boundary and size checks. |
| N1 | Remove the Play-logo invisible hit target; BAC-0001 probes its old rectangle. |
| N2 | Hide the zero-entry arrow, retain the one-entry menu, and require separate accessible/focusable controls and pointer-free activation in BAC-0002. |
| N3 | [BAC-0003](../bac/BAC-0003-play-selected-version.md) distinguishes a missing installed entry with and without catalog_id; the Install pick is that optional catalogue ID, never the installed ID by assumption. |
| N4 | Clear an open menu on busy/Quest transitions; BAC-0004 covers starting, running, selected/unrelated jobs, STOP, progress/CANCEL and return to PC. |

## Acceptance criteria and handoff

- [BAC-0001](../bac/BAC-0001-play-row-layout.md): compact Play row, selected-name band and removal of the logo, hidden hit target and standalone picker.
- [BAC-0002](../bac/BAC-0002-play-version-arrow.md): disjoint arrow/menu behavior, accessibility and selection.
- [BAC-0003](../bac/BAC-0003-play-selected-version.md): main action uses the resolved target and existing Install/preflight routes.
- [BAC-0004](../bac/BAC-0004-play-platform-and-busy.md): Quest and busy-state transitions.

This is a design-only revision. Astra rechecks the revised contract before implementation. The reference images do not prove the proposed split rendering, and no new behavior or build result is claimed here.
