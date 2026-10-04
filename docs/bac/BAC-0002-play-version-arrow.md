# BAC-0002: The Play arrow selects a PC version

Status: Proposed. Governing design: [S0-7 play split](../design/s0-7-play-split.md).

Given PCVR mode with at least two selectable versions and no running game or active job, clicking Play's down-arrow region opens a version list containing the installed and catalogue choices, the current choice visibly checked, and the Install another version entry. Clicking the main Play region does not open that list. Choosing a different version closes the list, updates the checked choice and visible status, and persists the selection after a page revisit or launcher restart. Choosing Install another version opens the Install page.

Verification: drive the two distinct hit regions and a version choice, then reload persisted state. A single large Play hit target that launches when the arrow is clicked fails this BAC. The existing `play_version_menu` snapshot route must still show the menu anchored to the arrow.
