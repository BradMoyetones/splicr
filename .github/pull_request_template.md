## What and why

<!-- What was wrong or missing, and what this changes. Link the issue: "Fixes #12". -->

## How it was checked

<!-- Tests added, files tried, systems run on. Screenshots for anything people see. -->

## Checklist

- [ ] `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` and `cargo xtask layers` pass
- [ ] New logic has tests; a crash fix has a test with the file that caused it
- [ ] New interface text goes through `tr!` and has its row in `apps/splicr/locales/es.tsv`
- [ ] Code or assets from elsewhere keep their licence and have a row in `ATTRIBUTION.md`
- [ ] Docs (`README.md`, `ROADMAP.md`, `docs/VISION.md`, `CLAUDE.md`) updated if this changes what they say
