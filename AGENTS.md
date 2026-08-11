# AGENTS.md

## Purpose and Scope

This file is a repo-level operating guide for general coding agents working in the Rover workspace. Use it to get oriented quickly, choose the right edit targets, and apply the expected verification and release guardrails.

This file supplements, and does not replace, the primary project docs:

- `README.md`
- `CONTRIBUTING.md`
- `ARCHITECTURE.md`
- `tests/README.md`
- `RELEASE_CHECKLIST.md`

When those sources conflict with assumptions, follow the repo sources of truth.

## Repo Map

- `src/command/*`: CLI nouns, verbs, argument parsing, and command wiring.
- `src/cli.rs`, `src/lib.rs`, `src/command/output.rs`, `src/options`, `src/error`, `src/utils`: shared CLI behavior, output shaping, errors, options, and utilities.
- `crates/*`: shared libraries and lower-level functionality used by the main CLI.
- `tests/integration`: targeted CLI behavior tests.
- `tests/e2e`: slower end-to-end coverage and fixture-driven command tests.
- `installers/*`: npm and binstall packaging/install behavior.
- `docs/source/*`: published documentation sources.

Treat this repository as a Cargo workspace rooted at the `rover` binary crate. Shared logic often belongs in `crates/*` rather than directly in the top-level CLI crate.

## Planning Prerequisites

Before writing code for a non-trivial change, confirm there's an approved planning artifact backing it:

- **Internal contributors** (Apollo teammates, or when you have access to Apollo's internal Confluence/Jira): a linked Confluence PRD or One-Pager describing the problem and proposed approach.
- **External contributors**: a GitHub issue that's been triaged per `CONTRIBUTING.md`'s "Using issues" section — opened, and discussed/acknowledged by the Rover team, not just filed.

If neither is linked in the task or request, stop before implementing. Ask for the link, or offer to open a GitHub issue first if no internal doc exists. Exempt: typos, trivial bug fixes, doc corrections, and other obviously-scoped changes with no design space to discuss.

When reviewing a PR, check the description for a linked Confluence PRD/One-Pager or a referenced GitHub issue (e.g. `Fixes #123`). Flag the PR if neither is present, unless the change is clearly trivial.

## How to Work in the Codebase

- Prefer targeted edits over broad refactors unless the task explicitly requires wider cleanup.
- Prefer targeted tests before full-workspace runs so feedback stays fast.
- Use the local cargo aliases when they help:
  - `cargo rover ...` runs the CLI from source.
  - `cargo xtask ...` runs workspace automation helpers.
- Keep new CLI surface aligned with the existing `rover [noun] [verb]` model. Before adding or restructuring commands, read `ARCHITECTURE.md`.
- Command implementations usually live under `src/command/<noun>/...`; shared behavior should be placed in the narrowest reusable module.
- If a change affects packaging, installation, or release behavior, inspect the relevant files under `installers/*`, docs, and CI workflows before editing.

## Structuring Pull Requests

Large or multi-concern changes should ship as a stack of small, independently reviewable PRs rather than one large PR. Split along two axes:

- **Vertical (dependency order):** within a single feature, land the lowest layer first — shared/foundational code, then the crate-level implementation (`crates/*`) that exposes the new capability, then the consumer that wires it up (typically `src/command/*`, but any downstream crate or binary counts). Each layer should be buildable and reviewable on its own: a foundation PR adds `pub` items that stay unused until a later PR consumes them (no dead-code warnings expected), an implementation PR is reviewable purely on the correctness of the new logic, and a consumer PR is reviewable purely on wiring and UX.
- **Horizontal (independent concerns):** when a task bundles multiple unrelated features or entities that only share the foundation layer, give each its own vertical slice instead of interleaving them into one PR, even if that's more convenient to write.

Stacked-PR tooling (e.g. the `gh-stack` skill / `gh stack` CLI) only supports **linear** stacks — one parent and one child per branch. Two horizontal slices that share a foundation must be modeled as **separate stacks**, both rooted at the foundation branch (`gh stack init --base <foundation-branch>` for the second slice), not as one branching stack.

When reviewing a PR, flag it if it bundles multiple unrelated vertical slices, mixes layers with no dependency reason to be mixed, or could otherwise be cleanly split along the axes above — and suggest the split in the review.

## Verification Expectations

Choose the smallest verification that proves the change, then scale up when the impact is broad.

- Fast local verification:
  - Run targeted tests such as `cargo test <target>` or package-specific commands.
  - Use focused test paths for narrow changes in a crate or command area.
- Standard workspace verification:
  - `cargo test --locked --workspace --exclude regressions`
- Lint:
  - `cargo clippy --locked --workspace --all-features --all-targets`
- Format check:
  - `cargo +nightly fmt --all -- --check`
- CI-parity checks when relevant:
  - `mise run test`
  - `mise run check`
  - `mise run cargo-deny`

Heavier checks should be used when the change touches cross-platform behavior, installers, composition, or shell-sensitive flows:

- `tests/e2e` coverage is intentionally heavier than normal local tests.
- Smoke-style checks exist to catch OS and shell differences that targeted local runs may miss.

## Docs, Changelog, and Release Guardrails

- Add a `CHANGELOG.md` entry under `Unreleased` for user-visible changes.
- Do not hand-edit `docs/source/contributing.md` below its autogenerated marker. It is regenerated by `cargo xtask prep`.
- Treat `docs/source/errors.md` as generated content as well; do not casually hand-edit generated sections.
- Release/versioning work has extra steps. Use `RELEASE_CHECKLIST.md` as the source of truth.
- `cargo xtask prep` is a release-prep command, not part of normal feature work.
- When doing release/versioning work, expect related updates beyond code changes, including generated docs, README/help output refreshes, and installer or documentation version updates where the checklist requires them.

## Cross-Platform and E2E Cautions

- Rover supports Unix and Windows. Avoid shell syntax, path handling, or test assumptions that only work on one platform.
- Read `tests/README.md` before changing e2e coverage or fixtures. It documents fixture usage, ignored e2e runs, and local prerequisites.
- Some e2e flows require Node.js and Git, and some Apollo-hosted scenarios may require Apollo-internal credentials.
- Smoke tests exist specifically to catch OS and shell differences. Use them when working on installer flows, command execution behavior, or cross-platform regressions.
