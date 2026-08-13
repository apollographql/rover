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

## Architecture and Implementation Patterns

- **`rover-client` implementations should use the Tower `Service` pattern.** Wrap the concern in a small `Clone` struct that holds an `inner: S` and implements `tower::Service<Req>`, delegating `poll_ready` to `inner` and building/mapping the request inside `call`. See `crates/rover-client/src/operations/graph/check/service.rs`'s `GraphCheck<S>` (or the sibling `service.rs` files under `crates/rover-client/src/operations/*/`) as the canonical shape. This keeps each operation as a thin, generic layer instead of a concrete HTTP call, so cross-cutting behaviors (retries, timeouts, rate limiting) can be layered on top with `tower::ServiceBuilder` in the calling command rather than duplicated inside every operation. `src/command/auth/whoami/mod.rs` shows the composition side: it builds a `ReqwestService` (from `rover-http`) and layers `RetryLayer`/`TimeoutLayer` via `ServiceBuilder` before handing the resulting generic service to the business logic. `crates/rover-http` holds the reusable layers (`retry.rs`, `timeout.rs`, `reqwest.rs`); `crates/rover-tower` holds small generic Tower helpers.
- **Compose retry and timeout explicitly for network-calling commands and operations; don't rely on the ambient client timeout alone.** The shared HTTP client configuration already applies a floor timeout from the global `--client-timeout` flag, and one internal code path already bakes a `RetryLayer` into the GraphQL operations that go through it — but that's a partial, inconsistent baseline: it has no per-attempt `TimeoutLayer`, and commands that build their service directly instead get neither retry nor an explicit Tower timeout, only the bare reqwest-level floor. When adding or touching a network-calling operation or command, compose `rover-http`'s `RetryLayer`/`TimeoutLayer` explicitly via `ServiceBuilder`: nest a short per-attempt `TimeoutLayer` inside a `RetryLayer` whose retry budget is the longer elapsed-time window, so a single hung attempt can't consume the entire retry budget. Don't reach for tower's own built-in `ServiceBuilder::timeout()` — use `rover-http`'s `TimeoutLayer` so timeout behavior stays consistent and greppable across the codebase. When reviewing a PR that adds or touches a network-calling command or operation, flag it if it relies solely on the ambient `--client-timeout` floor with no explicit retry/timeout composition, or if it introduces a new ad hoc timeout/retry mechanism instead of reusing `rover-http`'s layers.
- **There's no shared "default retry+timeout service" constructor yet — don't invent one speculatively.** Both existing composition sites (`whoami`, the `cli.rs` client-credentials flow) hand-roll the same `ServiceBuilder` chain with hardcoded constants. If a change needs this pattern in a third place, that's a signal a shared helper (in `rover-http`, or as a `StudioClientConfig` method) is due — propose it as its own small foundation-layer PR rather than copy-pasting a fourth hand-rolled chain, and get it reviewed on its own before consumers depend on it.
- **Each `rover-client` operation's Tower `Service` should expose its own default timeout/retry configuration, not just inherit a one-size-fits-all constant.** A `service.rs` wrapper (`crates/rover-client/src/operations/<noun>/<verb>/service.rs`) already owns how that operation builds and maps its request; it should also own a sensible default `RetryLayer`/`TimeoutLayer` configuration for that specific operation, exposed so callers can use it as-is or override it via `ServiceBuilder` per the retry/timeout composition guidance above. Where real observed latency data for that operation is available, use it to set the default — operations with meaningfully different latency profiles shouldn't share a timeout constant. Where that data isn't available or practical to pull in, use a conservative default and say so in a comment rather than copying another operation's constant unexamined.
- **Each `rover-client` GraphQL operation gets one module folder**, under `crates/rover-client/src/operations/<noun>/<verb>/`. `mod.rs` should hold the single `#[derive(GraphQLQuery)]` for that operation's query/mutation plus the types it exports; `service.rs` should hold only the `tower::Service` wrapper described above. This is the target shape — most existing operations instead split the derive into `service.rs` and exported types into a separate `types.rs`; treat that as the pattern to move away from, not to copy, when touching an operation. Many existing operations also have a `runner.rs` with a plain `pub async fn run(input, client: &StudioClient) -> Result<...>` that builds the default, unlayered service and calls it, so simple callers don't have to construct the service themselves. Keep `runner.rs` where it already exists for backwards compatibility with current call sites, but don't add a new one for new operations — new callers that need injected layers (retries, timeouts) should build and compose the service directly, the way `src/command/auth/whoami/mod.rs` does, rather than going through a runner wrapper.
- **Printing to the user goes through `rover-print` (or, in older code, the `rover-std` print macros), not raw `println!`/`eprintln!`.** Updates to old code should be updated to use `rover-print`. `crates/rover-print` exposes `Print`/`PrintExt` traits with `stdout`/`stderr` implementations (including a `mock()` variant for tests); `crates/rover-std/src/print.rs`'s `infoln!`/`warnln!`/`errln!`/`successln!`/`debugln!` macros are the older equivalent still used in some commands. Avoid adding new raw `println!`/`eprintln!` calls in command code — they bypass styling, the mockable print traits, and any future stdout/stderr redirection.
- **CLI output should be implemented via the `CliOutput` trait, not new `RoverOutput` variants.** `RoverOutput` (`src/command/output.rs`) is a large legacy enum with one variant per output shape, matched centrally to render `text()`/`json()`; new commands should instead define a dedicated output struct and implement the `CliOutput` trait (`text()`, `json()`, `exit_code()`) directly on it. Model new commands on `src/command/client/check/output.rs`, `src/command/auth/whoami/output.rs`, or `src/command/persisted_queries/generate/output.rs`, not on older commands still returning `RoverOutput::SomeVariant` (e.g. `src/command/explain.rs`, `src/command/contract/describe.rs`). Don't add new variants to `RoverOutput` for new commands.
- **Depend on abstractions, not concretions.** Command and operation structs should take trait bounds or injected services (Tower `Service` impls, the `Print` trait, etc.) rather than hardcoding concrete types like a specific HTTP client. This is what lets tests inject mocks (e.g. `rover-print`'s `mock()` printers, `mockall`-generated mocks) and lets callers inject retry/timeout-wrapped services without touching business logic. `src/command/auth/whoami/mod.rs` and the `crates/rover-client/src/operations/*/service.rs` files generic over `S: Service<...>` are the reference examples.

## Structuring Pull Requests

Large or multi-concern changes should ship as a stack of small, independently reviewable PRs rather than one large PR. Split along two axes:

- **Vertical (dependency order):** within a single feature, land the lowest layer first — shared/foundational code, then the crate-level implementation (`crates/*`) that exposes the new capability, then the consumer that wires it up (typically `src/command/*`, but any downstream crate or binary counts). Each layer should be buildable and reviewable on its own: a foundation PR adds `pub` items that stay unused until a later PR consumes them (no dead-code warnings expected), an implementation PR is reviewable purely on the correctness of the new logic, and a consumer PR is reviewable purely on wiring and UX.
- **Horizontal (independent concerns):** when a task bundles multiple unrelated features or entities that only share the foundation layer, give each its own vertical slice instead of interleaving them into one PR, even if that's more convenient to write.

Stacked-PR tooling (e.g. the `gh-stack` skill / `gh stack` CLI) only supports **linear** stacks — one parent and one child per branch. Two horizontal slices that share a foundation must be modeled as **separate stacks**, both rooted at the foundation branch (`gh stack init --base <foundation-branch>` for the second slice), not as one branching stack.

When reviewing a PR, flag it if it bundles multiple unrelated vertical slices, mixes layers with no dependency reason to be mixed, or could otherwise be cleanly split along the axes above — and suggest the split in the review. Also flag a PR that mixes `docs/source/*` changes with code changes; see "Docs, Changelog, and Release Guardrails" for why they need separate PRs.

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

## Testing Conventions

- **JSON output must satisfy both human and machine/agent readers through the existing envelope, not a bespoke shape.** Every command's `json()` implementation should go through the documented `{"json_version", "data", "error"}` contract (`src/options/output.rs`, `src/command/output.rs`'s `CliOutput` trait, documented in `docs/source/configuring.mdx`). `data` should be self-describing enough that a human skimming `rover ... --format json | jq` and a script/agent matching on `error.code` can both use it without reading source — don't invent a new top-level shape or bury meaningful fields in a message string.
- **Use `insta` snapshot tests to verify JSON output structure, including in `tests/e2e`.** This pattern is already established for `tests/integration` — `assert_json_snapshot!`/`assert_snapshot!` against checked-in `.snap` files, with the `cargo insta review`/`INSTA_UPDATE=always` workflow for updating them. Extend the same pattern to `tests/e2e` when adding or touching e2e coverage of a command's JSON output — don't hand-roll ad hoc field-by-field assertions there instead.
- **Use `rstest` for fixtures, `speculoos` for assertions — both are already the dominant style; keep it that way.** When asserting with `speculoos`'s `assert_that!`, assert on the full object or the full string (`.is_equal_to(...)` against a complete expected value), not just `.is_ok()`/`.is_some()`/`.contains(...)`. A shallow assertion (result is `Ok`, string contains a substring) passes even when the rest of the value silently regresses — asserting the entire rendered output (table, JSON value, etc.) is the model to follow.
- **Use Tower mocks to stub unrelated dependencies; reserve `httpmock` for tests that need to verify the actual HTTP layer.** `tower_test::mock` and the `rover_tower::mock_service!` helper (`crates/rover-http/src/test.rs`) are the right tool when a test only needs a fake inner `Service` so it can focus on the operation/business logic being tested. Reach for `httpmock`'s `MockServer` only where the test's actual purpose is verifying wire-level HTTP behavior (headers, status codes, retry/timeout interaction, request bodies) — i.e. integration-style tests, not as a default substitute for a Tower mock in a unit test.
- **Use `mise run coverage` (`cargo tarpaulin`, installed via `mise.toml`) to check coverage of the code you touched.** This is not wired into CI — it's a local verification step for agents, not an existing pipeline. Scope it to the crate/module you changed rather than the whole workspace, e.g. `mise run coverage -- --packages rover-client`, and use it to find untested branches in your own diff, not as a gate to justify writing low-value tests to hit a number.

## Docs, Changelog, and Release Guardrails

- Add a `CHANGELOG.md` entry under `Unreleased` for user-visible changes.
- Do not hand-edit `docs/source/contributing.md` below its autogenerated marker. It is regenerated by `cargo xtask prep`.
- Treat `docs/source/errors.md` as generated content as well; do not casually hand-edit generated sections.
- Keep `docs/source/*` changes in their own PR, separate from code changes. Docs ship on merge to `main` outside the normal release process (PRs that only touch `docs/` are exempt from the changelog-entry check), so bundling docs with code either forces an unreviewed doc live early or blocks a doc fix behind code review.
- Release/versioning work has extra steps. Use `RELEASE_CHECKLIST.md` as the source of truth.
- `cargo xtask prep` is a release-prep command, not part of normal feature work.
- When doing release/versioning work, expect related updates beyond code changes, including generated docs, README/help output refreshes, and installer or documentation version updates where the checklist requires them.

## Cross-Platform and E2E Cautions

- Rover supports Unix and Windows. Avoid shell syntax, path handling, or test assumptions that only work on one platform.
- Read `tests/README.md` before changing e2e coverage or fixtures. It documents fixture usage, ignored e2e runs, and local prerequisites.
- Some e2e flows require Node.js and Git, and some Apollo-hosted scenarios may require Apollo-internal credentials.
- Smoke tests exist specifically to catch OS and shell differences. Use them when working on installer flows, command execution behavior, or cross-platform regressions.
