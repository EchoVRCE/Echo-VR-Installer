# ADR-0001: Ownership of `bin/win10/BugSplat64.dll`

Status: **OPEN — no implementation choice approved**
Decision owners: Andrew and Mia
Scope: the live Windows PCVR game build. Linux/Proton, flat mode, event builds, and Quest need separate evidence before they inherit this decision.

## Decision needed

The game imports `BugSplat64.dll` before `WinMain`. Both EchoLoader 2 and NEVR Runtime use that exact file as their entry point, so they cannot each own the path independently. Andrew and Mia must decide which component owns the installed file, who may replace it, and how users move between old and new installations before the launcher installs NEVR. This draft records options and required consequences; it selects none.

## Evidence and current behavior

- The launcher identifies the slot as stock, EchoLoader 1/2, or `Unknown` using a stock hash and `ECHOLOADER_ID:` marker. It does not identify NEVR (`src/core/launcher/mods.rs:31-94` @ `ad9f709`). The Mods page edits EchoLoader JSON and reads `loader-status.json` (`src/core/launcher/mods.rs:255-279`, `src/core/launcher/mods.rs:775-799` @ `ad9f709`). Its documented community update owns shipped loader/config files (`src/core/launcher/mods.rs:1-15`, `docs/launcher/mods.md:10-26` @ `ad9f709`).
- PC Update deletes and replaces paths listed by the fetched manifest, excluding only explicit `keep` paths; a listed DLL can therefore be overwritten (`src/core/pc_update.rs:65-149`, `src/core/launcher/versions.rs:425-439` @ `ad9f709`). Verify reports differing manifest `add` files (`src/core/launcher/versions.rs:442-466` @ `ad9f709`); archive Repair can restore differing build files (`src/core/launcher/versions.rs:307-361` @ `ad9f709`). Whether a current remote manifest actually names this DLL was not verified here. The overwrite is a conditional risk, not an observed live update.
- A managed version's Remove deletes its guarded version folder; an external version is forgotten rather than deleted (`src/core/launcher/versions.rs:469-492` @ `ad9f709`). Mods REMOVE deletes only launcher-added plugins, not shipped files or the loader (`docs/launcher/mods.md:23-26` @ `ad9f709`). This ADR must distinguish removal of NEVR from removal of a game version.
- NEVR builds `BugSplat64.dll` for the same imported slot and documents backing up the previous file as `.original` before replacement (`nevr-runtime/README.md:14-23`, `nevr-runtime/docs/beta/INSTALL.md:15-22` @ `32ad7b0`). NEVR reads an ordered `plugins:` list from `config.yaml`; absent entries load no plugins (`nevr-runtime/src/core/nevr_config.cpp:209-240`, `nevr-runtime/src/runtime/ext/plugin_loader.cpp:93-113` @ `32ad7b0`). EchoLoader's JSON overlay is not that format. Binary compatibility of existing EchoLoader plugins with NEVR is unproven.
- NEVR's release distribution has a required-signing mode and verifies signer/root fingerprints, but the launcher has no agreed release source or pinned signer policy for installing it (`nevr-runtime/CMakeLists.txt:312-340`, `nevr-runtime/tools/build_distribution.py:160-171` @ `32ad7b0`). Artifact trust is ADR-0002, a separate prerequisite.

The cited source files are unchanged between the `launcher-m1` base and the original analysis commit `0a85ed9`; citations here point to the actual PR base `ad9f709` so reviewers can verify each claim in this branch.

## Options

| Option | Ownership and benefit | Cost and unresolved proof |
|---|---|---|
| **A. NEVR owns the slot.** | Install NEVR's signed DLL at the import path; it keeps its earliest hook point and one entry point. | Community updates and archive repair must cede the slot to NEVR when selected. Mods UI needs a NEVR configuration path (ADR-0003), and EchoLoader plugins need explicit compatibility or migration proof. Existing EchoLoader users need a reversible handover. |
| **B. EchoLoader owns the slot and loads NEVR separately.** | Retains the current loader, update, Verify, and Mods contract at the slot. | NEVR currently ships as the imported `BugSplat64.dll`; no cited source establishes that it can be chainloaded as a plugin or later DLL, or retain its pre-`WinMain` guarantees. This option needs runtime design and a real compatibility test before approval. |
| **C. The launcher manages an exclusive slot choice.** | Users can select a verified EchoLoader or NEVR owner per supported version; a recorded owner can make switching and rollback explicit. | The launcher must coordinate with the community manifest and archive repair for both modes, store the selected owner durably, detect drift, and maintain two configuration paths. It cannot simply label an unknown DLL as NEVR. |

Retaining the current EchoLoader-only behavior defers NEVR integration; it is the safe status quo until one option is approved, not a Stage 1 ownership answer.

## Required behavior under any approved option

| Operation | Contract that must be specified before implementation |
|---|---|
| **Update** | Identify the selected owner before applying a manifest. Under A, exclude the NEVR-owned slot from EchoLoader/community replacement and update it only from the trusted NEVR source. Under B, the manifest may continue to own EchoLoader's slot but NEVR's separate artifact needs its own update rule. Under C, route each slot update by the recorded owner. A failed download, validation, or replacement must leave a recoverable previous owner. |
| **Verify** | Check the installed slot against the selected owner's trusted artifact, and report `Unknown` or mismatched ownership distinctly from ordinary game-file drift. Under A/C, a community-manifest mismatch must not be presented as permission to overwrite NEVR. Under B, Verify may retain the EchoLoader check and separately check NEVR's artifact. |
| **Repair** | Restore the selected owner's verified bytes and configuration, not whichever DLL is in a generic archive or manifest. Under A/C, guard the slot from archive repair; under B, restore EchoLoader at the slot and repair NEVR separately. Preserve user plugins and configuration, and surface a partial failure so retry or rollback is possible. |
| **Remove** | Removing NEVR from an otherwise retained game must restore a known, backed-up previous owner or a verified stock file, with a defined answer when neither exists. Mods REMOVE still applies only to user-added plugins. Removing a managed game version may delete its guarded folder as today; forgetting an external install must not delete its files or silently restore a DLL in someone else's folder. |

The current updater can modify files before a later failure (`src/core/pc_update.rs:85-149` @ `ad9f709`); the recovery mechanism for this slot must therefore be explicit. This ADR does not claim that a transaction or rollback exists today.

## Migration and rollback questions for the owners

1. Detect stock, EchoLoader 1, EchoLoader 2 open/pinned, NEVR, and unknown bytes before touching the slot. What durable identity proves NEVR, and what happens to an unknown/custom DLL? Proposed safety floor: refuse automatic replacement of unknown bytes and preserve them for a human decision.
2. How is the previous exact DLL saved without overwriting an existing `BugSplat64.dll.original`? How is a failed switch reversed after each file/config write? NEVR's manual `.original` convention alone does not define repeat migrations.
3. Which EchoLoader `echoloader.local.json` settings and plugin files can be translated to NEVR's ordered `config.yaml`? Keep originals intact and mark unsupported plugins/settings; no compatibility is assumed from a matching `.dll` suffix or API number. ADR-0003 will settle the UI/config mapping.
4. Who changes the community manifest or the launcher's skip policy so an ordinary Update, Verify, or Repair cannot undo the selected owner? What happens when an older launcher encounters the migrated install?
5. Which source, signer and root fingerprints authenticate NEVR binaries? This must be answered in ADR-0002 before an install or repair path consumes a NEVR artifact.

## Consequences and handoff

BACs are pending because the owner, trust, and migration behavior remain undecided. No NEVR installation, slot classifier change, or update/repair exclusion is authorized by this OPEN draft. Andrew and Mia select an option and resolve the migration/rollback questions. Sol then records the decision and linked, testable BACs for Update, Verify, Repair, Remove, and migration; Astra reviews them before implementation. Stage 1 code waits for ADR-0002 artifact trust and ADR-0003 plugin configuration where those contracts apply.
