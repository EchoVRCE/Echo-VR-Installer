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
handy for checking UI changes.

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
