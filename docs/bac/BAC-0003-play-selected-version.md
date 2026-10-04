# BAC-0003: Main Play acts on the selected version

Status: Proposed. Governing design: [S0-7 play split](../design/s0-7-play-split.md).

Given PCVR with installed versions A and B at distinct paths, launch prerequisites satisfied, and B selected through the arrow, activating the main Play region routes B through the existing launch and preflight flow. It does not switch versions or open the list. Opening or closing the list alone launches and installs nothing. A test must observe B's resolved target/launch route, not merely that some game starts.

Given a selected catalogue entry C that is not installed, activating main opens the Install page with C's catalogue ID as install_pick and spawns no game. Given a selected installed entry whose files are missing and whose catalog_id is C, main opens Install with install_pick=C, not the installed entry ID. Given a missing external installed entry with no catalog_id, main opens Install with install_pick=None. Neither missing-entry case spawns a game. A later missing-executable error for a previously present installed target remains the existing launch guard; it is a separate fixture from an entry already classified as missing.

Verification: give A, B and C distinct IDs and paths, capture the resolved target and downstream route without starting a real game, and inspect Page::Install and install_pick in the two missing cases. A default-A launch, bypassed preflight, an assertion that the installed ID must always become install_pick, or a launch caused by merely opening the menu fails.
