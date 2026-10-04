---
name: write-bac
description: Write testable launcher behavior acceptance criteria in docs/bac, linked to an ADR and usable by implementation and review.
---

# Write a BAC

Read `AGENTS.md` and the governing ADR. Give each BAC a stable identifier and file in `docs/bac/`; link the ADR by path and identifier.

Describe observable starting state, action, and expected result so an implementer can write a test without guessing intent. Cover material error, repair, and recovery behavior as separate cases when they affect the decision. State platform or scope limits and any required fixture. Make the expected result distinguish the intended behavior from a plausible wrong implementation. Avoid implementation details unless the ADR makes them part of the contract.

Have Astra check the BAC through `review-design`. Update it when design review changes the contract; point Luna to the reviewed version.
