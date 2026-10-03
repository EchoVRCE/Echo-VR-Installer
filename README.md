# Echo VR Installer

This installs Echo VR onto your computer, **READ INSTRUCTIONS BELOW ON HOW TO INSTALL**

# Install instructions
Please join [this server](https://discord.com/invite/echo-vr-lounge) 

## Quest
After joining the server, look [here](https://discord.com/channels/779349159852769310/1408111261874393230)

## PCVR 
After joining the server, look [here](https://discord.com/channels/779349159852769310/1348139039139696752) 

**__If you need help with a specific error, go [here](https://discord.com/channels/779349159852769310/1135996306532356126)__**. 

However, before you ask for help, please consult the pinned messages/threads and or channels for help on your issue because there is a good chance that your error is already documented in one of those places within the server.

## Match links

The launcher joins matches from `spark://` and `https://echo.taxi/spark://…` links: paste one
into **Join** on the Play page or **Join from link** on the Servers page. On Windows and Linux
it also opens links clicked in Discord or the browser, if no other app (Spark) handles them
yet. **Open spark:// links** in Settings takes them over from Spark, or turns this off.
macOS can't pass links to the launcher, so paste them there.


## Already have Echo VR?

INSTALL looks for a copy first (the Meta app's libraries, `C:\EchoVR`, the launcher's own
library folder, and Wine prefixes on Linux) and offers **Use the copy on this PC**, or
**Choose echovr.exe** for one somewhere else. That copy is checked against the build's
checksums and added instead of downloading it again.

## SteamVR on Windows

The SteamVR choice runs Echo VR through [Revive](https://github.com/LibreVR/Revive)
(installed by the launcher) or, picked under Settings → Game, through
[EchoXR](https://github.com/EchoTools/EchoXR)'s OpenXR layer in the game's folder: no
injection and no administrator rights (the Meta library's copy excepted). EchoXR runs
only the live build. See [docs/launcher/echoxr.md](docs/launcher/echoxr.md).

## Linux

The PC version plays on Linux through Steam. **SET UP** on the Play page downloads a
private GE-Proton and [EchoXR](https://github.com/EchoTools/EchoXR)'s OpenXR layer (it
answers Echo's Oculus calls over OpenXR, so SteamVR, Monado or WiVRn drive the headset),
reads Meta's Platform SDK loader and P2P library out of Meta's own runtime package, and
adds Echo VR to Steam as a non-Steam game (Steam restarts for that). PLAY then starts it
through Steam.
It needs an active OpenXR runtime (SteamVR, Monado or WiVRn) and its service running; no
OpenVR runtime is needed. Only the live build runs this way; the event builds don't yet.

## Mods

The **Mods** page shows the selected PC version's mod loader
([EchoLoader](https://github.com/marshmallow-mia/EchoVR_Mod_Loader), the game's
`BugSplat64.dll`) and what it loaded at the last start, lets you turn plugins and asset
patches on or off, set a plugin's arguments, start without mods, and install mods from the
catalogue on files.echovr.de or a DLL of your own. The launcher keeps its choices in
overlay files beside the community update's, so updates and Verify never undo them. See
[docs/launcher/mods.md](docs/launcher/mods.md).

## Building from source

The installer is written in Rust (GUI: [egui](https://github.com/emilk/egui)). With a stable
[Rust toolchain](https://rustup.rs):

```sh
cargo run              # debug build
cargo build --release  # target/release/EchoVR_Installer(.exe)
cargo test             # unit tests (add `-- --ignored` for the network tests)
```

The bundled `adb` lives in `assets/platform-tools/`; `scripts/fetch-platform-tools.sh <version>`
refreshes it from Google's platform-tools release. Logs are written to the per-user data
directory (`%LOCALAPPDATA%\EchoVR_Installer\logs` on Windows,
`~/Library/Application Support/EchoVR_Installer/logs` on macOS,
`~/.local/share/EchoVR_Installer/logs` on Linux).

`ECHOVR_SNAPSHOTS=<dir> cargo run` renders every screen to PNGs in `<dir>` and exits, which is
handy for checking UI changes. Add `ECHOVR_SNAPSHOTS_DEMO=1` to render the launcher with
two made-up versions (nothing is saved).

## License

Copyright (C) 2024-2026 the Echo VR Installer contributors

This program is free software: you can redistribute it and/or modify it under
the terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the [GNU General Public License](LICENSE) for more
details.

You should have received a copy of the GNU General Public License along with
this program. If not, see <https://www.gnu.org/licenses/>.
