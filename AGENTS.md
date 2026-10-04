# Launcher agent workflow

This file owns the repository-wide workflow. A role's task instructions live in the relevant `.agents/skills/*/SKILL.md`; `docs/agents/` and `.codex/agents/` only route to them. Read this file and the skill for the task before acting. Every implementation agent, including a Codex subagent, must be invoked through an implementation skill. Do not substitute a prose prompt or a role file for the skill.

Work from `origin/launcher-m1` and target `launcher-m1` for Stage 0 until maintainers choose another integration branch. Keep decisions in `docs/adr/`, observable acceptance criteria in `docs/bac/`, behavior in tests and code, and change history in commits and PRs. Link to a canonical rule by heading or path; do not copy it into a second instruction file.

## Before every sprint

Run [`.agents/skills/start-sprint/SKILL.md`](.agents/skills/start-sprint/SKILL.md) first. It owns the repository inventory, preservation, item branch, commit cadence, early push, and draft PR procedure. Complete its hygiene report before starting an item.
Every agent sets and drives its own Codex Goal, reports steps by courier, checks the inbox between steps, and is replaced rather than nudged if stalled.
When a user or agent driver corrects an agent, record the correction in durable instructions and apply it in later turns. Do not acknowledge a correction by affirming that the user or driver is right. Never claim an error was avoidable until after saving and verifying a concrete prevention rule for that failure, including corrections made in the current turn. Do not quote the user or driver in explanations, reports, or instructions; express the rationale in the agent's own words so it stands on its own.

## Handoffs

1. **Sol designs:** use `write-adr` and `write-bac`. Record open choices, especially those requiring Andrew and Mia, before implementation.
2. **Astra reviews design:** use `review-design`. Sol resolves blocking findings and requests another review when the contract changes.
3. **Luna implements:** use `implement-bac` against the reviewed ADR and BACs. A new design choice returns to Sol and Astra.
4. **Sol reviews code:** use `review-code` against the diff, ADR, BACs, and test evidence.
5. **Astra critiques every PR:** use `critique-pr` independently, including fixes to prior findings.

No handoff proceeds with an unresolved blocking finding. Maintainers decide when to merge.

## Verification and proof of done

From the repository root, run `cargo build --release` and `cargo test` for an implementation or a review claiming verification. Record each command, its exit status, the tested commit or working-tree state, and material failures or skipped checks in the PR. A passing count alone is not evidence that a BAC is covered: map every applicable BAC to a test or an explicit substitute check, with its result. New or changed behavior tests need an observed assertion failure against a deliberately broken implementation, followed by a pass after restoring the fix; record the change and both results. If a test cannot be made to fail this way, explain the limitation and the substitute check. Do not claim a check ran when it did not.

The recorded baseline at `ad9f709` (`origin/launcher-m1`) is **165 passed, 13 ignored**. Compare a new run against that pinned baseline and explain a difference; the current run and its command are the verification evidence. The repository has no single-command CI gate yet, so these two commands are the current required checks. Add a fuller gate only when its recipe and CI job exist together.

A PR is ready for maintainer merge consideration when the reviewed ADR and BACs are linked, required maintainer decisions are recorded, implementation evidence covers each BAC, both checks and their exit statuses are reported, Sol and Astra have reviewed the actual diff, and all blocking findings have a recorded resolution and recheck. Reviewers report findings with a path and location, the observed evidence, impact, severity or blocking status, and a concrete way to reproduce or falsify the claim. State review scope and any unexamined area; an empty findings list is not proof of full coverage.

## Skill routes

| Task | Skill |
|---|---|
| Start every sprint | `.agents/skills/start-sprint/SKILL.md` |
| Sol architecture decision | `.agents/skills/write-adr/SKILL.md` |
| Sol acceptance criteria | `.agents/skills/write-bac/SKILL.md` |
| Astra design review | `.agents/skills/review-design/SKILL.md` |
| Luna implementation | `.agents/skills/implement-bac/SKILL.md` |
| Sol code review | `.agents/skills/review-code/SKILL.md` |
| Astra PR critique | `.agents/skills/critique-pr/SKILL.md` |
