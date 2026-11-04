# Contributing to Splicr

Thanks for wanting to help. Splicr is a small project with a clear direction, so a short
conversation before a big change saves everyone time.

## Before you start

- **Bugs:** open an issue with the bug template. A file that reproduces it (with private data
  removed) is the most useful thing you can attach.
- **Ideas and features:** open an issue with the feature template first. [ROADMAP.md](ROADMAP.md)
  shows what is planned and in what order, and [docs/VISION.md](docs/VISION.md) explains the
  decisions behind it. A pull request that adds a feature nobody discussed may be declined, however
  good the code is.
- **Security problems:** don't open an issue. See [SECURITY.md](SECURITY.md).
- Issues may be written in English or Spanish.

## Making a change

1. Fork the repository and create a branch from `main`: `feat/<what>` for features, `fix/<what>`
   for bugs, `docs/<what>` or `chore/<what>` for the rest.
2. Read [CLAUDE.md](CLAUDE.md): despite its name, it is the rule book for every contributor,
   human or agent. In short:
   - files are hostile input: no crash, no unbounded allocation, a test for every crash fix;
   - never lose the user's data;
   - colours from the palette, text through `tr!` with its Spanish translation, icons from Lucide;
   - code, comments and docs in English.
3. Run what CI runs before pushing:

   ```sh
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   cargo xtask layers
   ```

4. Open a pull request against `main` and fill in its template. CI runs on Windows, macOS and
   Linux; the first time you contribute, a maintainer approves the run.

## What a good pull request looks like

- **One thing.** A fix, a feature, or a refactor; not all three.
- **Explains why.** The description says what was wrong or missing and how it was checked.
- **Comes with tests.** Logic goes where it can be tested without a window (`splicr-core`,
  `splicr-io`, `workspace.rs`, `router.rs`), and its tests come along.
- **Shows the look.** A screenshot or the design snapshots when it changes what people see.
- **Credits what it borrows.** Code or assets from another project keep their licence notice and
  get a row in [ATTRIBUTION.md](ATTRIBUTION.md). Only code under a licence compatible with MIT.

Pull requests are squash-merged: the title becomes the commit on `main`, so make it a plain
sentence about what changes ("Read semicolon-separated CSV files with quoted line breaks").

## Releases

Releases are made by the maintainer: a version bump by pull request, then a `v<version>` tag on
`main`, which builds, signs and publishes the packages (see the README's Releasing section). Tags
`v*` are protected: pushing one publishes an update to every installed copy.

## Licence

By contributing, you agree that your contributions are licensed under the project's
[MIT licence](LICENSE).
