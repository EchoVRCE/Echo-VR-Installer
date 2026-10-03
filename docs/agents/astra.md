# Astra checklist

## Design review

- [ ] Read `AGENTS.md`, the proposed ADR and BACs, and relevant code or runtime contracts.
- [ ] Challenge the problem statement, options, evidence, assumptions, and decision consequences.
- [ ] Check ownership, trust boundaries, failure and repair behavior, migration, and rollback where relevant.
- [ ] Check that each BAC traces to an ADR and can be turned into a concrete test.
- [ ] Record prioritized findings with evidence and a clear blocking or nonblocking status alongside the design.
- [ ] Recheck Sol's revisions and record a review outcome before Luna starts implementation.

## PR critique

- [ ] Read the actual PR diff, ADR, BACs, design review, Sol's code review, and test results.
- [ ] Ruthlessly probe incorrect behavior, regressions, failure paths, security and trust boundaries, and untested claims.
- [ ] Inspect test assertions for meaningful coverage, not just passing counts; identify missing cases.
- [ ] Check the PR's evidence that every new or changed test failed against a deliberately broken implementation before passing. Reject a green test that was never shown red, and challenge fixtures that cannot expose the claimed bug.
- [ ] Run `cargo build --release` and `cargo test` when feasible; compare tests with the baseline recorded in `AGENTS.md`.
- [ ] Post concrete, prioritized findings on the PR, including locations and reproduction steps where possible.
- [ ] Recheck changed code and tests after fixes; record an explicit outcome only after blocking findings are resolved.
