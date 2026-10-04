---
name: write-adr
description: Write or revise a launcher architecture decision record in docs/adr when a design choice needs explicit review or maintainer decision.
---

# Write an ADR

Read `AGENTS.md`, existing ADRs, and affected code or external contract. Use the next stable `ADR-NNNN` identifier under `docs/adr/`; do not renumber existing records.

State the problem and constraints with checkable evidence, viable options and tradeoffs, the chosen option or explicit open decision, and consequences for implementation, verification, repair, and migration where relevant. Name the owner of an unresolved decision. Link the BACs that verify the choice, or mark them pending with a reason. Distinguish current behavior from proposed behavior and cite the revision or artifact behind a material claim.

Hand the ADR and linked BACs to Sol for `review-design`. Record the review and resolution where a later implementer can find them. An open option is not an approved decision.
