# BAC-0003: Main Play acts on the selected version

Status: Proposed. Governing design: [S0-7 play split](../design/s0-7-play-split.md).

Given PCVR mode with two installed versions, launch prerequisites satisfied, and the second selected through Play's arrow, clicking the main Play region starts the second version through the existing launch and preflight flow; it does not switch versions or open the list. Given a selected catalogue version that is not installed, the main region follows the existing Install-page route for that version. Given a selected installed entry whose files are missing, the existing missing-files behavior remains in force. Opening or closing the version list alone does not launch or install anything.

Verification: assert the target ID passed to the launch or Install path for each fixture. A test that only checks that *some* version starts cannot distinguish a regression to the default version and does not satisfy this BAC.
