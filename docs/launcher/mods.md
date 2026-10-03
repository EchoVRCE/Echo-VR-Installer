# Mods in the launcher

The live PC build's plugins are loaded by
[EchoLoader](https://github.com/marshmallow-mia/EchoVR_Mod_Loader) 2, which is the game's
crash reporter, `bin/win10/BugSplat64.dll`, so it loads on every start path. Its files
and formats are specified in EchoVR_Mod_Loader's `docs/formats.md`; this page covers what
the launcher does with them (`src/core/launcher/mods.rs`, the Mods page
`src/ui/launcher/mods.rs`).

## Who writes what

The community update (`https://files.echovr.de/updates/update.manifest`) owns
`echoloader.json`, the plugins it ships and `asset_patches/manifest.json`: Update and
Repair put them back, Verify reports any change. The launcher never edits them. It writes
only:

| File (in `bin/win10`) | What the Mods page changes there |
|---|---|
| `echoloader.local.json` | "Start without mods" (`enabled`), a plugin on or off and its arguments (`overrides`), the plugins it added (`add`, each with its `sha256`) |
| `asset_patches/manifest.local.json` | every asset patch, or one of them, on or off |
| `plugins/<file>.dll` | only the plugins it installed (from the catalogue) or added (from disk) |

An entry the launcher adds carries a `launcher` key the loader ignores:
`{"catalog_id": "…", "version": "…"}` for a catalogue mod, `{"local": true}` for a DLL from
disk. That is what makes it removable: REMOVE deletes only such files, never one the update
ships. A DLL that sits in `plugins/` without being listed shows as "not loaded"; turning it
on adds it (with its checksum).

The page reads the loader's `plugin_logs/loader-status.json` (written at every start) for
what was loaded and why not, and tells the loader in the slot apart by its
`ECHOLOADER_ID:<version>:<build>` marker: EchoLoader 2 (open, or the locked "pinned"
build, whose plugins can't be changed), EchoLoader 1 (the old `dbgcore.dll`: the page
offers the update that brings the new one), the game's own crash reporter, or something
else. Mods are for the live build only: event builds run EchoRelay's patch in the loader's
place.

On Linux, Proton loads `BugSplat64.dll` from the game's folder by itself (Wine has no
builtin of it). Only while a version still has EchoLoader 1 does `--play` set
`WINEDLLOVERRIDES=dbgcore=n,b` so Wine takes that `dbgcore.dll` instead of its own.

"Upload logs" sends the loader's `loader.log` and `loader-status.json`, each plugin's
newest logs from its folder in `plugin_logs`, and the newest crash records
(`plugin_logs/crashes`).

## The mods catalogue

The launcher reads `https://files.echovr.de/launcher/mods.json` (this folder has the
current draft, which is also built in for when it can't be fetched):

```json
{ "schema": 1,
  "mods": [
    { "id": "combat-stats", "name": "Combat Stats", "summary": "One line on what it does.",
      "author": "…", "version": "0.3.1", "file": "CombatStats.dll",
      "url": "mods/CombatStats-0.3.1.dll", "sha256": "…", "size": 412000,
      "api": 5, "capabilities": ["observes-only"], "args": { "key": "value" },
      "homepage": "https://github.com/…", "shipped": false } ] }
```

- `id`: lowercase letters, digits, `.`, `-`, `_` (max 64).
- `file`: the plugin's file name in `plugins/`: letters, digits, `.`, `-`, `_`, ending
  in `.dll`; never `BugSplat64.dll`.
- `url`: relative to the download mirrors (`files.echovr.de` / `evr.echo.taxi`), or an
  absolute `https://` URL on one of them. Required, with `sha256`, unless `shipped`.
- `sha256`: the file's checksum. The launcher checks the download against it and writes it
  into the overlay, so the loader refuses the file if it changes later.
- `shipped`: the community update brings it; the page shows it (INSTALLED, or "with the
  update"), never downloads it.
- `version`: shown, and compared with the installed one: a different version offers
  UPDATE.
- `api`, `capabilities` (`observes-only`, `cosmetic`, `alters-gameplay`, `alters-rules`,
  `network`, `hooks-engine`), `args` (its default arguments), `homepage`, `author`,
  `summary`, `size`: optional, shown on the page.

An entry that breaks a rule is left out; the rest of the list is still used.
