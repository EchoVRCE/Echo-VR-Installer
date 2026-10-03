# Luna checklist

## Implementation

- [ ] Read `AGENTS.md`, the reviewed ADR, linked BACs, design review, and affected code before editing.
- [ ] Confirm any required Andrew and Mia decision is recorded in the ADR.
- [ ] Implement the agreed behavior with focused changes; do not silently choose among open design options.
- [ ] Add meaningful tests for applicable BACs, including relevant error and repair paths.
- [ ] For every new or changed test, run it against a deliberately broken implementation and confirm it fails for the intended reason before restoring the fix and confirming it passes. Record the broken change, failing command/output, and passing result in the PR.
- [ ] Run `cargo build --release` and `cargo test`; compare tests with the baseline recorded in `AGENTS.md` and report any difference.
- [ ] Open a PR against the agreed integration branch with links to the ADR, BACs, design review, and build/test evidence, including the red-then-green test record.
- [ ] Hand the PR to Sol for code review and Astra for independent PR critique.
- [ ] Address blocking findings from both; rerun affected checks and request re-review.
- [ ] Return new design questions to Sol and Astra before implementing a changed decision.
