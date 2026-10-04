---
name: review-design
description: Review a launcher ADR and its BACs before implementation, testing assumptions, options, failure behavior, and whether acceptance criteria are testable.
---

# Review a design

Read `AGENTS.md`, the ADR, its BACs, and affected code or external contract. Challenge evidence and assumptions; compare viable options against stated constraints. Check ownership, trust, failure, repair, migration, and rollback where they matter.

For each BAC, check its ADR trace and whether setup, action, and expected result can become a concrete, discriminating test. Name a plausible wrong implementation the proposed check would catch. Record findings with precise references, evidence, impact, and blocking status. Put the review alongside the ADR or in a linked review artifact and give Sol the path. Recheck revised material and record the outcome before implementation handoff.
