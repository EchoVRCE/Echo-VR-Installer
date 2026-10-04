---
name: review-code
description: Review a launcher implementation PR against its reviewed ADR, every applicable BAC, and the actual diff and test evidence. Use for Sol code review and re-review after fixes.
---

# Review implementation

Read `AGENTS.md`, the PR diff, reviewed ADR and BACs, design review, and Luna's verification record. Trace each applicable BAC to changed production behavior and a test or justified substitute check. Inspect failure and recovery paths, changed public contracts, and evidence that each new or changed behavior test went red for the intended reason and green after the fix. A passing count does not establish coverage.

Run the checks in `AGENTS.md` §Verification and proof of done when feasible. Report the exact commands, exit statuses, tested revision or tree state, and any unavailable check. Compare with the pinned baseline there only after identifying what the current run measured.

Record findings on the PR with path and location, evidence, impact, blocking status, and a falsifier or reproduction. Report review scope even when no findings remain. Recheck each blocking fix in code and tests, then record an explicit outcome. Hand the PR to Astra for independent `critique-pr` review.
