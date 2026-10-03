# Launcher agent workflow

Work from `origin/launcher-m1` and target `launcher-m1` for Stage 0 work until the maintainers choose another integration branch. Keep decisions and acceptance criteria in the repository so a new agent can pick up the work.

1. **Sol designs.** Write numbered ADRs in `docs/adr/` for architectural choices and numbered, testable BACs in `docs/bac/` for observable behavior. Each BAC links its governing ADR. Andrew and Mia resolve choices that need maintainer approval before implementation.
2. **Astra reviews the design.** Review the ADR and BACs for missing options, assumptions, safety and failure cases, and whether every BAC can become a test. Record findings with the design; Sol revises until blocking findings are resolved.
3. **Luna implements.** Work from the reviewed design and BACs. Add meaningful tests for changed behavior and report build and test results in the PR. If implementation exposes a design gap, return it to Sol for a design update and Astra for another design review.
4. **Sol reviews code.** Check the implementation against the ADR, every BAC, and the reported tests. Record findings on the PR; Luna addresses blocking findings.
5. **Astra critiques every PR.** Inspect the actual diff and tests independently and ruthlessly: probe failure paths, regressions, trust boundaries, and claims of coverage. Record findings on the PR; Luna addresses blocking findings, and both reviewers recheck affected work.

## Handoff gates

- **Design ready:** Sol has written the ADR and linked, testable BACs; any decision requiring Andrew and Mia is recorded.
- **Design review done:** Astra has recorded a review outcome, and Sol has resolved all blocking design findings.
- **Implementation ready:** Luna has implemented the reviewed behavior, supplied BAC test evidence, and reported `cargo build --release` and `cargo test` results or named the reason either could not pass or run.
- **Code review done:** Sol has reviewed the actual diff against the ADR and BACs and rechecked fixes for blocking findings.
- **PR critique done:** Astra has independently reviewed the implementation and tests, and rechecked fixes for blocking findings.

An implementation is done when every gate above is met and no blocking review finding remains. Maintainers decide when to merge. A PR should link the design, design review, implementation/test evidence, Sol code review, and Astra PR critique.

## Commands and baseline

Run from the repository root:

```sh
cargo build --release
cargo test
```

On `ad9f709` (`origin/launcher-m1`), the recorded test baseline is **165 passed, 13 ignored**. Compare later runs with that baseline and name any changed count or failure.

## Canonical role checklists and skills

- Sol: `docs/agents/sol.md`; skills: `.agents/skills/write-adr/`, `.agents/skills/write-bac/`.
- Astra: `docs/agents/astra.md`; skills: `.agents/skills/review-design/`, `.agents/skills/critique-pr/`.
- Luna: `docs/agents/luna.md`.

The role files are the checklists. `.codex/agents/*.toml` only routes agents to them.
