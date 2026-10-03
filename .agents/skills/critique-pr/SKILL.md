---
name: critique-pr
description: Independently critique a launcher PR and its tests against reviewed ADRs and BACs, with concrete blocking findings and a recheck after fixes.
---

# Critique a PR

Read `AGENTS.md`, `docs/agents/astra.md`, the full PR diff, linked ADRs and BACs, design review, Sol's code review, and reported checks. Inspect the actual changed code and tests; do not infer coverage from a green count.

Probe each applicable BAC, regression risks, failure and repair paths, trust boundaries, and assertions that could pass for incorrect behavior. Check PR evidence that every new or changed test failed for the intended reason on a deliberately broken implementation before passing; reject a green test that was never shown red. Check that fixtures differ on the dimension of the claimed bug, such as orientation. Run `cargo build --release` and `cargo test` when feasible, and compare with the baseline recorded in `AGENTS.md`. Name any check you could not run.

Post prioritized findings on the PR with file locations, evidence, impact, and a clear blocking status. Recheck the specific fixes and tests, then record an explicit outcome. An unresolved blocker keeps the critique open.
