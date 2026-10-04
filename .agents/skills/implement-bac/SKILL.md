---
name: implement-bac
description: Implement a reviewed evr-launcher ADR and its linked behavior acceptance criteria, with discriminating tests and PR evidence. Use for Luna implementation work or any Codex implementation agent on this workflow.
---

# Implement reviewed behavior

Read `AGENTS.md` and the reviewed ADR, linked BACs, design review, and affected code before editing. Confirm any required Andrew and Mia decision is recorded. If the implementation needs a new design choice, return that question to Sol and Astra before choosing behavior.

Make a BAC-to-code-and-test map. Implement each applicable BAC, including material error, repair, and recovery behavior. Use focused changes. For every new or changed behavior test, show that a deliberate break of the relevant production behavior makes the intended assertion fail, then restore the implementation and show the test pass. The broken variant must reach the assertion; a compile failure or unrelated panic is not proof. Record the break, command, failure, and pass in the PR. If this method cannot test a case, name the limit and an independent substitute check.

Run the commands in `AGENTS.md` §Verification and proof of done. Record exact commands, exit statuses, commit or working-tree state, and any baseline difference. Commit only owned paths. Open a PR against the agreed integration branch linking the ADR, BACs, design review, BAC evidence, and check results. Ask Sol for `review-code` and Astra for `critique-pr`; address blockers and rerun affected checks before re-review.
