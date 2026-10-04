---
name: critique-pr
description: Independently critique a launcher PR and its tests against reviewed ADRs and BACs, with concrete blocking findings and a recheck after fixes.
---

# Critique a PR

Read `AGENTS.md`, the full PR diff, linked ADRs and BACs, design review, Sol's code review, and reported checks. Inspect changed code and tests; do not infer coverage from a green count.

Probe each applicable BAC, regression risks, failure and repair paths, trust boundaries, and assertions that could pass for incorrect behavior. Check the PR's red-to-green evidence for each new or changed behavior test: the deliberate break must trigger the intended assertion, and the fixture must differ on the dimension of the claimed bug. Run the checks in `AGENTS.md` §Verification and proof of done when feasible, and state the tested revision and any check that could not run.

Post prioritized findings on the PR with file location, evidence, impact, a falsifier or reproduction, and a clear blocking status. State review scope. Recheck specific fixes and tests, then record an explicit outcome; an unresolved blocker keeps the critique open.
