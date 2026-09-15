# PRD: Rover plugin system

- **Status:** Ready for Review
- **Owner:** Isaac Good
- **Component:** Rover CLI plugins (`supergraph`, `router`, `apollo-mcp-server`)
- **Jira:** ROVER-420 (epic), ROVER-426 (this PRD)
- **Product ask:** confirm or revise the plugin implementation, covering stable plugin versioning and transparency into when plugins are downloaded and used.
- **Related:** the companion internal PRD, "Rover Plugin Installation: Batch Install & Offline Guarantee." Adopted here as Phase 1; see [§6](#6-relationship-to-the-batch-install-prd).

## 1. Problem

Rover delegates composition, the local router, and the MCP server to separately released binaries that it downloads on demand. The architecture is sound. What users struggle with is how those binaries are versioned and managed:

1. **Versions are not stable by default.** Unless a user pins an exact version, every run resolves a floating alias such as `latest-2` against Apollo's registry. A new upstream release silently changes what a CI job composes with, and nothing in the repo records or diffs the change. Rover already warns that composition "will fail without an exact federation version" in the future, but the only way to comply today is to hand-pin three plugins with three different version syntaxes.
2. **Downloads are surprising and opaque.** The first `rover dev` or `rover supergraph compose` on a machine downloads a binary from a host many enterprise networks block. A cached run says nothing about which version it used. No command lists installed plugins, structured output never records which plugin ran, a failed download has no error code, and no single setting guarantees "do not touch the network for plugins" across every command that uses them.

The evidence is consistent across support tickets, GitHub issues, and customer calls:

- **Registry outages break CI** even when the needed plugin is already on disk ([#2687](https://github.com/apollographql/rover/issues/2687), plus recurring support cases). Rover 0.41 added a fallback to an installed plugin and a global skip-update switch, which softens the outage case but not the reproducibility gap.
- **Restricted networks cannot reach the registry.** One enterprise routes plugin downloads through an authenticated artifact mirror and lost two releases to a regression ([#3326](https://github.com/apollographql/rover/issues/3326)). Another can install Rover but not plugins, because its approved mirror serves the GitHub release layout rather than Rover's registry layout; its current workaround is to stop Rover from updating at all.
- **Offline and pre-provisioned environments have no supported path.** Offline caches ([#1638](https://github.com/apollographql/rover/issues/1638), open since 2023), read-only Lambda layers ([#1253](https://github.com/apollographql/rover/issues/1253)), and users who copied a binary into place and still saw re-downloads ([#1808](https://github.com/apollographql/rover/issues/1808)). The recurring request, in a customer's words: "no surprise downloads from another binary."
- **Docs describe behavior that no longer exists.** The install and compose docs still list Federation 1 versions that Rover now rejects, and two pages point at a plugin-versions file that has been deleted from the repo.

## 2. Decision: keep the plugin architecture

The alternative on the table was compiling composition into the Rover binary ([#3575](https://github.com/apollographql/rover/pull/3575), closed August 2026). We are **confirming** the plugin architecture because the composition version must remain the user's choice, independent of the Rover version: customers pin `federation_version` precisely so their supergraph output stays stable while Rover upgrades. The original motivation for compiling it in, retiring the JavaScript composition binary, is moot; the latest `supergraph` plugin is already native Rust.

What we are **revising** is the management around the architecture. Customers are not asking for fewer binaries. They are asking to declare up front what should be present, install it deliberately, and never have a build fetch anything they did not ask for. The model is `cargo install`: an explicit, idempotent install step, a record of what is installed, and layered user-level and project-level declarations.

## 3. Requirements

### Stable versioning

1. **Plugins can be declared at the user level, the project level, or both.** A user-level `~/.rover/` (the existing install location) and a project-level `.rover/` directory can each hold a manifest, `rover.yaml`, naming the expected plugins and versions, plus a lockfile, `plugin-versions.lock`, recording the exact version each floating alias resolved to. The manifest is named for Rover rather than for plugins so it can carry other project-level Rover configuration later; this PRD defines only its plugin section. The user-level lockfile is the record of what is installed on the machine. The project-level manifest and lockfile are committed, so every machine that installs against them gets an identical set of exact versions. When both declare the same plugin, the project wins inside that project; the user level fills in the rest. Rover finds the project directory by searching the working directory and its parents, as Cargo and git do, so no path flag is needed in the common case. Binaries install to the user-level location by default; a project's `rover.yaml` may redirect them to a project-local install root, as `cargo install --root` does, for vendored containers, Lambda layers, and per-project isolation.
2. **Every plugin-using command honors the declaration.** `supergraph compose`, `dev`, `connector`, and `lsp` use the locked exact versions without consulting the registry. Precedence, most specific first: explicit flags and environment variables, project declaration, user declaration, today's floating default.
3. **One version syntax for all three plugins:** `latest`, a bare major such as `2`, or an exact `=X.Y.Z`, accepted identically in the manifest, `rover install --plugin`, `supergraph.yaml`, and every flag and environment variable. Today's per-plugin forms (`latest-2`, `vX.Y.Z`) keep working as deprecated aliases so existing files are not broken. `rover dev` gains a `--router-version` flag alongside its existing composition and MCP version flags.

### Transparency

4. **Every run reports which plugin it used.** One line per plugin at default verbosity: name, exact version, and whether it was downloaded, already installed, or used as a fallback. The same facts appear in `--format json` output so CI can assert on the version that actually ran.
5. **Users can list installed plugins** with exact versions and locations, in text and JSON.
6. **Plugin failures are first-class errors.** Resolution, download, extraction, and never-download violations each have an error code, a suggestion naming the plugin and the fix, and appear under `error.code` in JSON output.
7. **The registry and mirror contract is documented**: which hosts Rover contacts, what a mirror must serve, how to pre-seed plugins in a CI image or container, and how to run fully offline. Stale Federation 1 content and dead links are removed.

### Control

8. **Every command can be told never to download plugins, and fails fast when it cannot comply.** `rover install` gets `--no-download` with an environment variable equivalent, as the batch-install PRD specifies. The on-the-fly commands keep `--skip-update` and `APOLLO_ROVER_SKIP_UPDATE`, which already mean "use what is installed, do not contact the registry." The two mechanisms stay separate because their jobs differ: one guards an explicit install step, the other guards a build. Both must fail immediately and name the missing plugin rather than downloading or silently succeeding, and the docs present them side by side as the offline toolkit.
9. **Batch, resumable installs with per-plugin reporting**, as specified in the batch-install PRD. Reinstalling an exact version that is already present is a no-op that says so; `--force` reinstalls. When `plugin-versions.lock` is present, `rover install` installs exactly what it records; floating aliases are re-resolved and the lockfile rewritten only on explicit request, as with `npm ci` versus `npm install`.
10. **Mirrors that serve the GitHub release layout work for plugins**, as they already do for installing Rover itself, without reproducing the registry's layout or headers.
11. **At Rover 1.0, plugin download becomes opt-in.** A command that needs a plugin not present locally stops and tells the user how to install it, unless the project or user has opted in to on-demand download. Requirements 1, 2, 8, and 9 are the migration path: by 1.0, declaring plugins in `rover.yaml` and running `rover install` is the documented default workflow, and on-demand download is the exception a user turns on.

### Integrity and compatibility

12. **Downloaded plugins are verified against a published checksum.** The registry publishes a checksum for every plugin artifact, and Rover refuses to install an artifact that does not match (ROVER-450). Both halves are in scope: the same team owns the registry and Rover.
13. **Plugin network traffic honors Rover's global HTTP settings** (`--client-timeout`, certificate flags, retry policy) in every step, including version resolution.
14. **Nothing already working regresses before 1.0.** A user with no manifest, lockfile, or new settings gets today's behavior plus the one new line from Requirement 4. Existing flags and environment variables keep working. The only planned breaking change is Requirement 11, at 1.0.

## 4. Non-goals

- A general package manager: no dependencies between plugins, no third-party plugins, no marketplace. The supported set stays `supergraph`, `router`, `apollo-mcp-server`.
- Compiling any plugin into the Rover binary (rejected in [§2](#2-decision-keep-the-plugin-architecture)).
- Distributing plugins as npm packages or bundling fallback binaries into the Rover install.
- musl composition support, which depends on the `supergraph` binary shipping musl builds.
- Changing how the registry publishes aliases or how Federation is released.
- Changing ELv2 license acceptance.
- Making plugin download opt-in before Rover 1.0. That change is committed for 1.0 (Requirement 11), not for any 0.x release.

## 5. Success metrics

- **Reproducibility.** Two clean machines installing from the same committed manifest and lockfile end up with identical plugin versions, and `rover supergraph compose --format json` on both reports the same composition version.
- **Offline guarantee.** With the never-download setting and pre-seeded plugins, every plugin-using command completes with zero outbound connections. With a plugin missing, it fails within one second with an error code naming the plugin.
- **Transparency.** Every plugin-using run prints plugin, exact version, and source, in text and JSON, covering the downloaded, cached, and fallback paths.
- **Support signal.** No new tickets of the shape "Rover wanted version X, I had Y, nothing told me" in the two quarters after Phases 1 and 2 ship. #1638 and #1253 are closable with a documented path.
- **Mirror adoption.** An enterprise whose approved mirror serves the GitHub release layout can install plugins through it without pinning versions via environment variables.
- **Docs honesty.** Nothing documented for plugins is rejected by the code.

## 6. Relationship to the batch-install PRD

The companion batch-install PRD is the approved design for the `rover install` surface, with tickets ROVER-445 through ROVER-450. This PRD adopts it as Phase 1 and does not restate it. It extends it in three ways: it brings the on-the-fly commands (`compose`, `dev`, `connector`, `lsp`) into the same model, which that PRD scoped out; it adds the lockfile requested in that PRD's review; and it resolves the manifest as `.rover/rover.yaml` at both the user and project level rather than `supergraph.yaml` or a new `.apollo/` directory.

## 7. Phasing

Each phase ships independently.

- **Phase 0, hygiene.** Remove stale Federation 1 docs and dead links; print the plugin-used line on cached and fallback runs; assign error codes to plugin failures; make version resolution honor the global HTTP settings. Requirements 4 (text), 6, 7 (cleanup), 13.
- **Phase 1, `rover install`.** ROVER-445 through ROVER-449 as filed. Requirement 9, and the `rover install` half of 8. Requirement 12 (ROVER-450) ships in two steps, registry publication first and then Rover verification, both by this team.
- **Phase 2, declared and reproducible plugins.** `rover.yaml` and `plugin-versions.lock` at both levels, honored by every command; fail-fast behavior for both offline switches; JSON provenance; plugin listing; unified version syntax and `--router-version`; the registry and mirror docs page. Requirements 1 through 5, 7, and 8.
- **Phase 3, mirrors.** GitHub-release-layout mirrors for plugin downloads. Requirement 10.
- **Rover 1.0.** Plugin download becomes opt-in (Requirement 11). Whether composition also requires an exact pin or lockfile, fulfilling the warning `rover supergraph compose` already prints, is decided in 1.0 planning.

All phases need end-to-end coverage that today does not exist for the `dev` and `compose` plugin paths, and the offline switches need a network-denied test environment.

## 8. Decisions log

Questions raised while drafting, and how they were settled. Alternatives are listed so reviewers can reopen one deliberately rather than by accident.

- **Where declarations live.** `.rover/` at both the user and project level, over `supergraph.yaml` (graph-specific, shared with composition) and a new `.apollo/` directory. Manifest `rover.yaml`, named for Rover so it can carry other project configuration later; lockfile `plugin-versions.lock`.
- **Discovery.** Search upward from the working directory, as Cargo and git do, over current-directory-only or an explicit path flag.
- **Install root.** A project may redirect it, as `cargo install --root` does, over always installing to the user-level location.
- **Offline switches.** `--no-download` on `rover install` and `--skip-update` on the other commands stay separate, matching the batch-install PRD, over a single new environment variable or extending `APOLLO_ROVER_SKIP_UPDATE` to cover `rover install`.
- **Version syntax.** `latest`, bare major, `=X.Y.Z` for all plugins with deprecated aliases, over adopting the `supergraph` plugin's `latest-N` form everywhere.
- **Lock refresh.** Only on explicit request, over refreshing every run or when the manifest changes.
- **Opt-in download.** Committed for Rover 1.0, over leaving it undecided or keeping opt-out indefinitely.
- **Checksums.** Kept as a requirement covering both registry publication and Rover verification, since this team owns both, over dropping it from this PRD.
