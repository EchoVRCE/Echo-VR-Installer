# BAC-0001: Play row occupies the top of the main panel

Status: Proposed. Governing design: [S0-7 play split](../design/s0-7-play-split.md).

Given the Play page at the supported default window size, when it renders in PCVR or Quest mode, the Play action, Check for Updates, and PCVR/Quest toggle appear together at the top of the main content panel, where the large Echo VR logo previously sat. The large logo and separate VERSION dropdown/caption are absent. The selected-version/status line remains readable, and the action row does not overlap Community News or Server Info. The Install page retains its own existing hero layout.

Verification: compare Play screenshots in both platform modes and the Install screenshot, and inspect the interactive regions. A screenshot that merely hides VERSION but leaves the controls below an empty logo area fails this BAC.
