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


## Linux

The PC version plays on Linux through Steam. **SET UP** on the Play page downloads a
private GE-Proton and [EchoXR](https://github.com/EchoTools/EchoXR)'s OpenXR layer (it
answers Echo's Oculus calls over OpenXR, so SteamVR, Monado or WiVRn drive the headset),
reads Meta's Platform SDK loader out of Meta's own runtime package, and adds Echo VR to
Steam as a non-Steam game (Steam restarts for that). PLAY then starts it through Steam.
It needs an active OpenXR runtime and a registered OpenVR runtime (SteamVR, or xrizer /
OpenComposite on Monado and WiVRn): Proton only turns OpenXR on when OpenVR is there.
Only the live build runs this way; the event builds don't yet.

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
