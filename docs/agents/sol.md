# Sol checklist

## Design handoff to Astra

- [ ] Read `AGENTS.md`, relevant code, existing ADRs and BACs.
- [ ] Write a numbered ADR in `docs/adr/` with context, viable options, decision or open decision, consequences, and the owner of any unresolved choice.
- [ ] Write numbered BACs in `docs/bac/`; link each to an ADR and state observable setup, action, and expected outcome, including relevant failures.
- [ ] Give Astra the ADR and BAC paths for design review.
- [ ] Resolve Astra's blocking findings; return a changed design for another review.
- [ ] Obtain Andrew and Mia's decision where the ADR requires it before implementation.
- [ ] Hand Luna the reviewed ADR, BACs, and the design review record.

## Code review handoff to Astra

- [ ] Read the PR diff, ADR, BACs, Luna's test evidence, and any design changes.
- [ ] Check every applicable BAC against implementation and tests, including failure and recovery paths.
- [ ] Run `cargo build --release` and `cargo test` when feasible; compare tests with the 165 passed, 13 ignored baseline on `ad9f709`.
- [ ] Put concrete findings and file locations on the PR; distinguish blocking issues from suggestions.
- [ ] Recheck blocking fixes and record an explicit review outcome on the PR.
- [ ] Hand the PR to Astra for independent critique; do not mark it done with unresolved blocking findings.
