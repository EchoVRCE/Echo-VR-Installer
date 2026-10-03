---
name: write-adr
description: Write or revise a launcher architecture decision record in docs/adr when a design choice needs explicit review or maintainer decision.
---

# Write an ADR

Read `AGENTS.md`, `docs/agents/sol.md`, existing ADRs, and the affected code or external contract. Use the next stable `ADR-NNNN` identifier under `docs/adr/`; do not renumber existing records.

State the problem and constraints with evidence, the viable options and their tradeoffs, the chosen option or an explicit open decision, and consequences for implementation, verification, repair, and migration where relevant. Name the owner of any unresolved decision. Link the BACs that will verify the choice, or mark them pending with a reason.

Hand the ADR and linked BACs to Astra for design review. Record the review and resolution where a later implementer can find them. Do not present an open option as an approved decision.
