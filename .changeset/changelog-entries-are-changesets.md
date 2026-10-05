---
category: maint
breaking: false
authors:
- dotdat
---

Changelog entries are written as changesets in `.changeset/`

Each pull request adds one file to `.changeset/` instead of editing `CHANGELOG.md`, so open pull requests no longer conflict over the changelog. `cargo xtask changeset release` writes each release's `CHANGELOG.md` section and GitHub release notes from them. CI requires a changeset unless a pull request only changes `docs/` or carries the `skip-changeset` label. See `.changeset/README.md`.
