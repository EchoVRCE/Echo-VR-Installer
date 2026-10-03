---
name: review-design
description: Review a launcher ADR and its BACs before implementation, testing assumptions, options, failure behavior, and whether acceptance criteria are testable.
---

# Review a design

Read `AGENTS.md`, `docs/agents/astra.md`, the ADR, its BACs, and the affected code or external contract. Challenge evidence and assumptions; compare the options against the stated constraints. Check ownership, trust, failure, repair, migration, and rollback where they matter.

For each BAC, check its ADR trace and whether setup, action, and expected result can become a concrete test. Record findings with precise references and priority, distinguishing blockers from suggestions. Put the review alongside the ADR or in a linked review artifact and give Sol the path. Recheck revised material and record the outcome before implementation handoff.
