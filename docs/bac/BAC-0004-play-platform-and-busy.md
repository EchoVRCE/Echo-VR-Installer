# BAC-0004: Split behavior respects platform and busy state

Status: Proposed. Governing design: [S0-7 play split](../design/s0-7-play-split.md).

Given idle PCVR with the version menu open, transition separately into (1) the launch-start interval before the process is reported running, (2) an owned running game, (3) an externally running game, (4) a selected-target job and (5) an unrelated launcher job. At each transition the popup closes, its temporary open flag is cleared, the arrow is disabled with the applicable reason, no menu item can be selected, and the saved selection does not change. On return to idle PCVR the arrow may reopen a fresh menu; it does not restore a stale open popup.

Main and blue actions keep their independent state: an owned running game can still expose STOP while version switching is disabled; a selected-target job shows progress in the main segment and its applicable CANCEL action in the blue segment; an unrelated job follows its existing main/update restrictions. The disabled arrow cannot dispatch STOP, Play, Update or Cancel. Progress fill, percentage and action labels remain within their assigned regions at the default, minimum and wide viewports.

Given an open PCVR menu, switch to Quest and back. Quest immediately clears the PC menu flag and displays no PC arrow or entries while its Play, Check for Updates and platform controls retain their Quest actions. Returning to PCVR leaves the menu closed and restores the prior selected PC ID and visible name. A platform switch never changes the selected PC version by itself.

Verification: drive each transition from an already open menu, inspect the open flag and visible popup, attempt pointer and keyboard arrow activation, and separately exercise STOP and applicable progress/CANCEL. Repeat the platform transition and check the selected ID/name. A test that only enters Play already busy, allows a stale popup after Quest, disables STOP with the arrow, or loses the saved PC choice fails.
