# Launcher catalogues

The launcher reads `https://files.echovr.de/launcher/versions.json` (this folder has the
current draft). Until it is published, a built-in list with the same content is used.

Rules the launcher enforces (see `src/core/launcher/catalog.rs`):

- `id`: lowercase letters, digits, `.`, `-`, `_` (max 64). It becomes the install folder
  name inside the library, so it must never change for a published version.
- `url` / `data_url`: either a path relative to the download mirrors
  (`files.echovr.de` / `evr.echo.taxi`, the fastest is picked), or an absolute `https://`
  URL on one of those hosts.
- `update_manifest`: absolute `https://` URL on those hosts, in the usual
  `add <path> <sha256>` / `del <path>` format. Applied after install and by "Update".
- `sha256`: optional; when set, the downloaded zip must match before it is extracted.
- `size`: optional, bytes, shown in the list.
- `hosted`: optional, `"live"` or `"event"` for builds the community's main servers run.
  The Install page lists these first, with a red LIVE BUILD or orange EVENT BUILD tag,
  above a divider; everything else goes below it. Unknown values count as not hosted.
- `summary`: optional, one short line shown next to the name in the Install page's list.

To publish a new build side by side with the current one, add an entry with a new `id`,
its own zip and (if it gets updates) its own update manifest.
