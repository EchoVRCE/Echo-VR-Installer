# BAC-0004: Split behavior respects platform and busy state

Status: Proposed. Governing design: [S0-7 play split](../design/s0-7-play-split.md).

Given PCVR mode while Echo VR is running or a launcher job is active, Play's arrow cannot change versions and explains the reason, as the current VERSION picker does. The existing main action, including Stop or job progress/cancel behavior, remains available according to its current state. Given Quest mode, the PC version list and its arrow are absent; Play, Check for Updates, and the PCVR/Quest toggle retain their Quest actions. Switching back to PCVR restores the previously selected PC version.

Verification: exercise running, active-job, Quest, and return-to-PCVR fixtures. A menu that opens during a running game or Quest mode fails this BAC even if no selection is ultimately saved.
