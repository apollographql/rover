# Spec: Rover plugin system

- **PRD:** [prd.md](./prd.md)
- **Jira:** ROVER-420 (epic), ROVER-426 (PRD)
- **Status:** Draft
- **Owner:** Isaac Good

## 1. Purpose and scope

This spec defines the observable contract for how Rover declares, resolves, installs, verifies, reports on, and refuses to download plugin binaries. It covers every PRD requirement (R1–R14) across all phases, including the Rover 1.0 breaking change.

It describes what Rover must do under which conditions, and the exact user-facing surface involved: file formats, flags, environment variables, message text, JSON fields, and error classes. It does not describe how any of it is built; implementation approach belongs in a separate implementation plan, one per phase.

Requirements are tagged with the PRD requirement they realize, e.g. `(R1)`.

## 2. Terminology

- **Plugin**: one of exactly three binaries Rover delegates to — `supergraph`, `router`, `apollo-mcp-server`. No other name is valid anywhere in this contract.
- **Plugin-using command**: any command that needs a plugin to do its work — `rover supergraph compose`, `rover dev`, `rover connector`, `rover lsp` — plus the `rover plugin` verbs, which act on plugins deliberately rather than on the fly.
- **On-the-fly resolution**: a plugin-using command other than a `rover plugin` verb obtaining a plugin during a run.
- **Registry**: the Apollo-operated host Rover contacts to resolve floating version aliases and download plugin artifacts. Its default host is overridable via `APOLLO_ROVER_DOWNLOAD_HOST`.
- **Floating alias**: a version request that does not name an exact version, and therefore must be resolved against the registry to become one — `latest` or a bare major.
- **Exact version**: a full semantic version, written `=X.Y.Z`.
- **Manifest**: `rover.yaml`, a user-authored declaration of which plugins and version requests a machine or project expects.
- **Lockfile**: `plugin-versions.lock`, a Rover-authored record of the exact version each request resolved to.
- **Global level**: the Rover home directory, `~/.rover/` by default, relocatable via `APOLLO_CONFIG_HOME` as today. Shared by every project on the machine.
- **Project level**: the nearest ancestor directory of the working directory that contains a `.rover/` directory, or the directory containing the manifest named by `--manifest-path`.
- **Install root**: the directory whose `bin/` subdirectory holds installed plugin binaries. Each level has one, and it is the directory holding that level's manifest: `<project>/.rover/bin/` and `~/.rover/bin/` by default.
- **Source**: how a run obtained the plugin it used — `downloaded`, `installed`, or `fallback`.

---

## 3. Functional requirements

### 3.1 Version grammar (R3)

- **FR1**: Rover must accept exactly three version forms for every plugin, in every place a plugin version can be written:
  - `latest` — the newest release of that plugin, any major.
  - a bare major, e.g. `2` — the newest release within that major.
  - `=X.Y.Z` — that exact release.
- **FR2**: The grammar must be accepted identically in all of: the manifest, `rover plugin install <name>@<version>`, `supergraph.yaml`'s `federation_version`, and every plugin version flag and environment variable. No plugin may accept a form another plugin rejects.
- **FR3**: The following legacy forms must continue to parse, with identical resolution behavior to their modern equivalent: `latest-0`, `latest-1`, `latest-2` (→ `latest` within that major, i.e. the bare major), and `vX.Y.Z` (→ `=X.Y.Z`).
- **FR4**: Each legacy form must, on use, emit one deprecation warning naming the modern equivalent, at most once per invocation per distinct form.

  Required text:
  > Warning: `latest-2` is a deprecated version format. Use `2` instead.

  > Warning: `v1.2.3` is a deprecated version format. Use `=1.2.3` instead.
- **FR5**: An unparseable version must be rejected before any network call, with an error naming the offending value, the plugin, and the three accepted forms.
- **FR6**: Federation 1 version requests must continue to be rejected as they are today. `latest-0`, `latest-1`, and any `=0.x.y`/`=1.x.y` request for the `supergraph` plugin parse (FR3) and then fail the existing Federation 1 rejection, not a grammar error.
- **FR7**: `rover dev` must accept `--router-version`, taking the FR1 grammar, alongside its existing `--federation-version` and `--mcp-version`. It must be honored identically to the existing `APOLLO_ROVER_DEV_ROUTER_VERSION` environment variable, with the flag winning when both are set.

### 3.2 Manifest (R1)

- **FR8**: Rover must read a manifest named `rover.yaml` from a `.rover/` directory at the global level and/or the project level. Neither is required; absence is not an error.
- **FR9**: The manifest's plugin section must have this shape, where each value is any FR1 version form:

  ```yaml
  plugins:
    supergraph: "2"
    router: "=2.1.0"
    apollo-mcp-server: latest
  ```

- **FR10**: The manifest is named for Rover, not for plugins, so that it can carry unrelated project configuration later. Rover must ignore top-level keys it does not recognize rather than erroring, so that a manifest written for a newer Rover does not break an older one. It must not ignore unrecognized keys *inside* `plugins:`; an unknown plugin name is an error naming the three valid names.
- **FR11**: A manifest that is not valid YAML, or whose `plugins` section is not a mapping of plugin name to version string, must fail with an error naming the file's path and the parse problem. A malformed manifest must never be silently skipped and must never be treated as absent.
- **FR12**: A manifest at either level may redirect that level's install root:

  ```yaml
  install_root: ../vendor/rover
  ```

  A relative path resolves against the directory containing the manifest, not the working directory. Binaries then live under `<install_root>/bin/`. A project manifest's `install_root` redirects only that project; a global manifest's redirects the global level for every project on the machine.

### 3.3 Lockfile (R1, R9)

- **FR13**: Rover must maintain a lockfile named `plugin-versions.lock` next to the manifest that produced it, at the same level. Its shape:

  ```yaml
  version: 1
  plugins:
    - name: supergraph
      requested: "2"
      resolved: 2.9.3
      checksum: "sha256:<hex>"
  ```

  `requested` records the version form that produced this entry, so a reader can see whether a floating alias was resolved. `checksum` records the verified artifact digest (FR69) and is omitted for entries locked before checksums were published.
- **FR14**: A level's lockfile is the record of what is installed at that level. The project lockfile records the project's plugins, the global lockfile records the machine's.
- **FR15**: Rover must update a level's lockfile on every successful install or removal at that level, and at no other time. An ordinary plugin-using run must never write a lockfile.
- **FR16**: Installing one plugin must not re-resolve any other. A floating alias already recorded in the lockfile keeps its `resolved` version unless it is named on the command line or `rover plugin resolve` is run (FR37). This is what makes a committed project lockfile stable.
- **FR17**: A lockfile whose `version` Rover does not recognize must fail with an error stating the file was written by a newer Rover, not be ignored or overwritten.
- **FR18**: When a manifest and its sibling lockfile disagree — a plugin is declared but absent from the lock, or a declared exact version differs from the locked `resolved` version — a plugin-using command must fail rather than silently re-resolving.

  Required text:
  > The plugin lockfile is out of date with `rover.yaml`: `router` is declared as `=2.2.0` but locked at `2.1.0`. Run `rover plugin resolve` to update it.

### 3.4 Discovery and layering (R1)

- **FR19**: Absent `--manifest-path` (FR25), Rover must find the project level by searching the working directory and then each ancestor directory in turn for a `.rover/` directory, stopping at the first one found, as Cargo and git do. It must not search past the filesystem root, and must not treat `~/.rover/` found this way as a project — a working directory under the home directory resolves to "no project," not to the global level as a project.
- **FR20**: If the first `.rover/` found contains neither a manifest nor a lockfile, the search stops there anyway; Rover must not continue upward looking for a populated one. An empty `.rover/` directory is a valid, if empty, project.
- **FR21**: When a plugin is declared at both levels, the project declaration wins for that plugin inside that project. Layering is per plugin, not per file: a project declaring only `router` still gets the global level's `supergraph` declaration.
- **FR22**: When no project is discovered, the project level is simply absent: only global declarations apply, and plugin-using commands resolve and look up globally. Rover must never invent a project by treating the working directory as one. No command creates a `.rover/` directory except `rover plugin install --manifest-path` (FR25).

### 3.5 Install location (R1)

Rover follows npm's local-first model rather than Cargo's global-first one: a project's plugins belong to the project, and installing for the whole machine is the explicit, flagged case.

The duplication this implies is accepted. `rover plugin install` is not a prerequisite step and is never invoked implicitly by another command, so unlike `npm install` it is not run once per project as a matter of course. The expected common path is a single global install that every project then resolves against via FR26, with a project root created only by a user who deliberately wants one — typically for reproducibility or an offline image. Per-project copies are therefore opt-in in practice as well as in principle, and the spec does not trade the simpler model away to avoid a cost most users will not pay.

- **FR23**: When a project is in scope, `rover plugin install` must install into the **project** install root by default. No flag is required to get project-local plugins. When no project is in scope, it must install into the **global** install root, as it does today.
- **FR24**: `rover plugin install --global` (short form `-g`) must install into the global install root instead. `APOLLO_ROVER_GLOBAL` (`1` or `true`) is its environment-variable equivalent, for CI images that cannot pass flags. The global root it targets is the one a global manifest's `install_root` names, if any (FR12); a project's `install_root` has no effect under `--global`.
- **FR25**: Every plugin-using command must accept `--manifest-path <FILE>` (short form `-m`), naming a project manifest directly and overriding discovery (FR19). The named file's directory is the project level: its install root is that directory's `bin/` unless `install_root` redirects it (FR12), and its lockfile is `plugin-versions.lock` alongside it (FR13).
  - For `rover plugin install`, a path whose file does not exist yet must be created, along with its parent directories and the rest of the project root. This is the only way a project root comes into existence, and the explicit counterpart to `--global`. `--manifest-path` and `--global` together are an error.
  - For every other plugin-using command, a path that does not exist must fail with the malformed-declaration error code (FR60.6) naming the path. These commands never create anything.
  - The spelling matches `rover persisted-queries generate --manifest-path <FILE>` and `cargo --manifest-path`. The two Rover flags name different kinds of manifest on unrelated commands; the shared spelling is intentional, since in both cases it means "use this declaration file instead of the one I would have found."
- **FR26**: Plugin-using commands must look up a needed plugin in the project install root first, then the global install root, and use the first match at the required version. A plugin installed globally therefore keeps working in a project that has not installed its own copy.
- **FR27**: When Rover creates a project root (FR25), it must also write a `.gitignore` alongside the manifest, ignoring `bin/`, so that binaries are not committed while `rover.yaml` and `plugin-versions.lock` are. Rover must not overwrite a `.gitignore` that is already there.
- **FR28**: A project install root must be self-contained: removing `<project>/.rover/bin/` and re-running `rover plugin install` must restore the project to a working state without touching the global level.

### 3.6 Resolution precedence (R2)

- **FR29**: For each plugin a command needs, Rover must determine the version request by taking the first of these that is present, most specific first:
  1. An explicit command-line argument: `--federation-version`, `--router-version`, `--mcp-version`, or a `rover plugin install` positional `<name>@<version>`.
  2. The corresponding environment variable (`APOLLO_ROVER_DEV_COMPOSITION_VERSION`, `APOLLO_ROVER_DEV_ROUTER_VERSION`, `APOLLO_ROVER_DEV_MCP_VERSION`).
  3. `supergraph.yaml`'s `federation_version`, for the `supergraph` plugin only.
  4. The project manifest.
  5. The global manifest.
  6. Today's built-in floating default for that plugin.
- **FR30**: `supergraph.yaml` outranks the manifest (FR29.3 over FR29.4) and is authoritative for the `supergraph` plugin. It is graph-specific, it is already the documented home of `federation_version`, and two consequences follow that the opposite ordering could not deliver: a repository may hold several `supergraph.yaml` files selected per invocation with `--supergraph-config`, each on its own Federation version, and adding a `rover.yaml` to an existing repository cannot silently change what that repository composes with. When both are present and they disagree, Rover must use `supergraph.yaml` and emit one warning per invocation.

  Required text:
  > Warning: `supergraph.yaml` sets `federation_version: =2.9.3`, overriding the `supergraph` version declared in `rover.yaml`.

- **FR31**: When the resolved request is a floating alias and a lockfile at the level that supplied the request has an entry for that plugin, the locked exact version must be used without contacting the registry (R2). A request from a flag, environment variable, or `supergraph.yaml` is not covered by a lockfile entry and resolves normally.
- **FR32**: Precedence must be identical across `supergraph compose`, `dev`, `connector`, `lsp`, and `install`. No command may apply its own ordering. Version precedence (this section) and install location (§3.5) are independent: a version taken from the global manifest still installs into the project root.

### 3.7 The `plugin` noun (R9)

- **FR33**: Plugin management must live under a `plugin` noun, matching Rover's `rover [noun] [verb]` model, with exactly these verbs:
  - `rover plugin install [<name>@<version>]...` — install the named plugins, or everything declared in scope when none are named.
  - `rover plugin list` — list what is installed (FR42).
  - `rover plugin uninstall <name>[@=<version>]...` — remove installed plugins (FR43).
  - `rover plugin resolve` — re-resolve floating aliases and rewrite the lockfile (FR37).

  Each verb accepts `--global`/`-g` (FR24), `--manifest-path`/`-m` (FR25), and `--format json`. `install` and `resolve` additionally accept `--no-download` (FR48). `uninstall` never touches the network under any flag.
- **FR34**: `rover install --plugin <name>@<version>` must keep working as a deprecated alias for `rover plugin install <name>@<version>`, including its ELv2 acceptance flags, and must emit one deprecation warning naming the replacement. Bare `rover install`, which installs Rover itself, is untouched by this spec.

  Required text:
  > Warning: `rover install --plugin` is deprecated. Use `rover plugin install supergraph@=2.9.3` instead.

- **FR35**: `rover plugin install` must accept any number of `<name>@<version>` positional arguments, installing each in one invocation. With none, and a manifest or lockfile in scope, it must install every plugin declared there.
- **FR36**: With a lockfile in scope, `rover plugin install` installs exactly the `resolved` versions it records and makes no resolution request to the registry, the way `npm ci` does. A locked version the registry no longer serves fails per FR61; Rover must not silently substitute a neighbouring version.
- **FR37**: `rover plugin resolve` must re-resolve every floating alias in the in-scope manifest against the registry, install the results, and rewrite the lockfile at the target level. It is the only operation that rewrites a lockfile wholesale. Because it must reach the registry by definition, `rover plugin resolve --no-download` is a contradiction and must be rejected before any work begins.
- **FR38**: A named `rover plugin install <name>@<version>` must record what it installed at the target level, as `npm install <pkg>` does: the plugin and its requested version are written to that level's `rover.yaml`, and the resolved exact version to its `plugin-versions.lock`. Both files are created if absent, along with the `.rover/` directory and its `.gitignore` (FR27). `--no-save` must suppress the manifest write while still updating the lockfile.
- **FR39**: Installing an exact version already present at the target level must be a no-op that says so and exits 0.

  Required text:
  > `supergraph` v2.9.3 is already installed in this project.

  > `supergraph` v2.9.3 is already installed globally.

- **FR40**: `--force` must reinstall a plugin that is already present at the target level, replacing the binary on disk. It must not touch the other level.
- **FR41**: A batch install must be resumable: each plugin's outcome is reported as it completes, and re-running after a partial failure must skip the plugins that already succeeded. A failure on one plugin must not abort the remaining ones; every plugin is attempted, and the command exits non-zero if any failed.
- **FR42**: `rover plugin list` must list installed plugins with exact version, install location, and level, in text and in JSON. Inside a project it lists both levels, marking which copy a plugin-using command would actually use per FR26. `--global` restricts it to the global level.
- **FR43**: `rover plugin uninstall <name>...` must remove the named plugins' binaries from the target level, which is chosen exactly as it is for `install`: the project when one is in scope, the global level otherwise, overridden by `--global` or `--manifest-path` (FR23–FR25).
- **FR44**: A bare `<name>` must remove every installed version of that plugin at the target level. `<name>@=X.Y.Z` must remove only that version. A floating alias is ambiguous here and must be rejected before anything is removed.

  Required text:
  > `router@2` is ambiguous for uninstall. Name the plugin alone to remove every installed version, or an exact `router@=2.1.0`.

- **FR45**: Uninstalling must remove the plugin's entry from the target level's manifest and lockfile, mirroring what a named install writes (FR38). `--no-save` must leave the manifest entry in place while still updating the lockfile — which deliberately creates the FR18 drift state, so Rover must warn when it does.

  Required text:
  > Warning: `supergraph` is still declared in `rover.yaml`. The next plugin-using command will fail until you remove it or re-install.

- **FR46**: Uninstalling must never touch a level other than the target. In particular, uninstalling in a project that has no copy of its own must not remove the global copy it was falling back to (FR26); it must report that nothing was removed at this level and name `--global`.

  Required text:
  > `supergraph` isn't installed in this project. It resolves from the global install; re-run with `--global` to remove it there.

- **FR47**: Uninstalling a plugin that is installed at no level must be a no-op that says so and exits 0, mirroring FR39.

### 3.8 Never-download controls (R8)

- **FR48**: `rover plugin install` must accept `--no-download`, with the environment variable equivalent `APOLLO_ROVER_NO_DOWNLOAD` (accepting `1` or `true`). Under it, the command must make no outbound connection at all — neither resolution nor artifact download.
- **FR49**: The on-the-fly commands must continue to honor `--skip-update` and `APOLLO_ROVER_SKIP_UPDATE`, which mean "use what is installed, do not contact the registry."
- **FR50**: The two mechanisms stay separate and neither implies the other: `--no-download` guards an explicit install step, `--skip-update` guards a build. `APOLLO_ROVER_SKIP_UPDATE` must not suppress downloads for an explicit `rover plugin install`, and `APOLLO_ROVER_NO_DOWNLOAD` must not alter on-the-fly behavior.
- **FR51**: Under either control, a plugin that is present at neither level must fail immediately, before any network call, naming the plugin, the version it needed, both directories searched, and the control in force. It must never download, and must never silently succeed with a different version.

  Required text:
  > Rover needs the `supergraph` plugin v2.9.3, but it isn't installed in `/work/app/.rover/bin` or `/home/me/.rover/bin` and downloads are disabled by `--no-download`.

- **FR52**: Under `--skip-update`, a floating request that has no lockfile entry must be satisfied by the newest installed version within the requested major, searching the project root before the global root (FR26). If none is installed at either level, FR51 applies.
- **FR53**: A run under either control must complete with zero outbound connections when every needed plugin is present. This is observable: with the registry unreachable, and with a network-denied environment, the run must still succeed.

### 3.9 Provenance reporting (R4)

- **FR54**: Every plugin-using run must report each plugin it used: name, exact version, and source. One line per plugin, at default verbosity, on stderr — including on runs where nothing was downloaded, which report nothing today.

  Required text, by source:
  > Using the `supergraph` plugin v2.9.3 (downloaded).

  > Using the `supergraph` plugin v2.9.3 (already installed).

  > Using the `supergraph` plugin v2.9.3 (fallback: couldn't reach the plugin registry).

- **FR55**: The reported version must always be the exact version that ran, never the request. A run that asked for `2` reports the resolved `2.9.3`.
- **FR56**: The line must be printed once per plugin per invocation. A long-running session (`rover dev`, `lsp`) must not reprint it on recomposition or hot reload, unless the plugin actually changes — a mid-session `federation_version` change that swaps the binary prints a new line for the new version.
- **FR57**: The same facts must appear in `--format json` output, under `data`, for every plugin-using command:

  ```json
  {
    "json_version": "1",
    "data": {
      "plugins": [
        {"name": "supergraph", "version": "2.9.3", "source": "downloaded", "level": "project", "path": "/work/app/.rover/bin/supergraph-v2.9.3"}
      ]
    },
    "error": null
  }
  ```

  `source` is exactly one of `downloaded`, `installed`, `fallback`; `level` is exactly one of `project`, `global`. Both fields must be present on success and on failure, listing the plugins resolved before the failure.
- **FR58**: The stderr lines must be printed regardless of output format. stderr is not the machine-readable channel, and CI logs are where the fallback case needs to be visible.
- **FR59**: The existing fallback warning must continue to be printed in addition to the FR54 line, since it tells the user something the provenance line does not: that the version may be stale.

### 3.10 Error contract (R6)

- **FR60**: Each of these failure classes must have its own stable error code, surfaced as `error.code` in JSON output and in the printed error:
  1. Version resolution failed (registry unreachable, alias unknown, no release matches the request).
  2. Artifact download failed (HTTP error, truncated transfer).
  3. Artifact extraction or installation failed (corrupt archive, unwritable install root).
  4. A never-download control was in force and the plugin was absent (FR51).
  5. Checksum verification failed (FR69).
  6. The manifest or lockfile is malformed, or the two disagree (FR11, FR17, FR18).
  7. An exact version that the registry no longer serves was requested (FR61).
- **FR61**: A request for an exact version the registry once served and now does not — a yanked or withdrawn release — must fail with its own error code, distinct from class 1's "no release matches," so that a script can tell a version that never existed from one that was taken away. The error must name the plugin, the version, and where the request came from, and must suggest a different version to install: the newest available release in the same major when the registry offers a listing, and `rover plugin resolve` when it does not.

  Required text:
  > The `supergraph` plugin v2.9.3, locked in `plugin-versions.lock`, is no longer available from the plugin registry. The newest available 2.x is v2.9.5. Run `rover plugin install supergraph@=2.9.5`, or `rover plugin resolve` to re-resolve every declared plugin.

- **FR62**: Every one of these errors must carry a suggestion that names the plugin and a concrete next step — not a generic "submit an issue." An unsupported architecture keeps its existing dedicated error rather than being folded into class 2.
- **FR63**: Error codes must be documented in the generated error reference, with the same page-per-code treatment every other Rover error code gets.

### 3.11 Network behavior (R13, R10)

- **FR64**: Every plugin network step — version resolution and artifact download alike — must honor Rover's global HTTP settings: `--client-timeout`, certificate flags, and proxy environment. Resolution must not bypass them, as it can today.
- **FR65**: Plugin requests must apply an explicit retry policy with a per-attempt timeout, so a single hung attempt cannot consume the whole budget. A resolution or download failure that is retried must not print a warning per attempt.
- **FR66**: Rover must be able to fetch plugin **artifacts** from a mirror that serves the GitHub release layout, configured the same way the existing Rover-install mirror support is, without that mirror reproducing the registry's URL layout or response headers.
- **FR67**: Version **resolution** always goes to the registry, mirror or no mirror. The registry can answer "the newest release within major X" for every plugin, and a mirror is not required to expose a release listing — only to serve an artifact at a known path. A configured mirror therefore changes where bytes come from, not how a version is chosen.
- **FR68**: The consequence must be stated rather than discovered: a mirror alone does not make a floating alias resolvable on a network that cannot reach the registry. On such a network, every request must already be exact — from a committed lockfile (FR31, FR36) or an explicit `=X.Y.Z` — and Rover must say so when a floating alias fails to resolve and a mirror is configured.

  Required text:
  > Couldn't reach the plugin registry to resolve `supergraph@2`. A mirror supplies plugin artifacts but not version resolution. Pin an exact version, or commit a lockfile with `rover plugin resolve` from a network that can reach the registry.

### 3.12 Integrity (R12)

- **FR69**: The registry must publish a checksum for every plugin artifact, and Rover must verify a downloaded artifact against it before installing. A mismatch must fail the install with the dedicated error code (FR60.5), must not leave a partial or unverified binary in the install root, and must not be overridable by a flag.
- **FR70**: Verification failure text must name the plugin, the version, the expected digest, and the digest computed.
- **FR71**: Until the registry publishes checksums for a given artifact, Rover must install it and record no `checksum` in the lockfile. Rover must not fail on an absent published checksum, or this requirement cannot ship before the registry half does. Once a lockfile entry has a checksum, a later install of that same entry that finds no published checksum must fail rather than downgrade silently.

### 3.13 Compatibility and migration (R14)

- **FR72**: A user with no manifest, no lockfile, and no new flags or environment variables must get today's behavior in every respect, plus the FR54 provenance line and any FR4 deprecation warnings. No existing flag, environment variable, or `supergraph.yaml` field may change meaning.
- **FR73**: The plugin binary layout and naming under an install root must be unchanged from today, so a pre-seeded container image, a Lambda layer, or a hand-copied binary keeps working. The project install root uses the same layout as the global one.
- **FR74**: FR26's project-then-global lookup order is not a breaking change and ships with Phase 2: a user with no project keeps resolving globally, and a user with a project gains a root that is empty until they install into it.
- **FR75**: FR23's install-target default is likewise not a breaking change, because Rover never creates a project root on its own (FR22, FR25). An existing user, an existing CI job, and an existing Dockerfile all have no `.rover/` in scope, so `rover plugin install` keeps targeting the global root exactly as it does today. Project-local installs reach only users who deliberately opted in by running `rover plugin install --manifest-path` or authoring a `.rover/rover.yaml`.
- **FR76**: FR77 is the only breaking change in this spec, and it lands at 1.0. Every other requirement must ship without breaking an existing invocation.

### 3.14 Rover 1.0 (R11)

- **FR77**: At Rover 1.0, a plugin-using command that needs a plugin present at neither level must stop and tell the user how to install it, rather than downloading it.

  Required text:
  > Rover needs the `supergraph` plugin v2.9.3, which isn't installed. Run `rover plugin install supergraph@=2.9.3`, or set `allow_automatic_download: true` in `rover.yaml` to let Rover download plugins on demand.

- **FR78**: Opt-in must be expressible at either level, as `allow_automatic_download: true` in the manifest, layered per FR21, and as the environment variable `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` (`1` or `true`), which outranks both manifests. With it set by either route, pre-1.0 on-demand behavior applies. The name says *automatic* because it governs only downloads a command performs on its own; an explicit `rover plugin install` is never automatic and is never gated by it (FR80).
- **FR79**: Per-invocation controls outrank the standing opt-in. When `allow_automatic_download` is set and `--skip-update` or `--no-download` is also in force, the never-download behavior wins and the command fails per FR51. A standing declaration may permit a download; it may never compel one.
- **FR80**: `rover plugin install` is unaffected by FR77 and FR78: it is the explicit install step and continues to download unless FR48 forbids it.

### 3.15 Documentation (R7)

- **FR81**: A documentation page must state which hosts Rover contacts for plugins, what a mirror must serve to satisfy FR66, how to pre-seed plugins into a CI image or container, and how to run fully offline using FR48–FR53.
- **FR82**: `--no-download` and `--skip-update` must be documented side by side as the offline toolkit, stating plainly that they guard different steps and that neither implies the other.
- **FR83**: The project-versus-global model must be documented explicitly: that a plain `rover plugin install` is global until a project root exists, that `--manifest-path` is what creates one and `--global` is what overrides one, what to commit (`rover.yaml`, `plugin-versions.lock`) and what not to (`.rover/bin/`), and the disk cost of per-project copies. The `rover install --plugin` alias must be documented as deprecated, pointing at `rover plugin install`.
- **FR84**: All Federation 1 version content must be removed from the install and compose docs, and links to the deleted plugin-versions file must be removed or repointed.
- **FR85**: No plugin behavior may be documented in `--help` or published docs that the code does not implement.

---

## 4. Acceptance criteria

**Project-local install is the default**
Given a project containing `.rover/`, when `rover plugin install supergraph@=2.9.3` runs anywhere inside it, then the binary is written under `<project>/.rover/bin/`, the global install root is untouched, `<project>/.rover/rover.yaml` gains `supergraph: "=2.9.3"`, and `<project>/.rover/plugin-versions.lock` records `resolved: 2.9.3`.

**Global install requires the flag**
Given the same project, when `rover plugin install supergraph@=2.9.3 --global` runs, then the binary is written under `~/.rover/bin/`, the global manifest and lockfile are updated, and nothing in `<project>/.rover/` changes. `APOLLO_ROVER_GLOBAL=true` with no flag must behave identically.

**Lookup prefers the project, falls back to global**
Given `supergraph` v2.9.3 installed globally and not in the project, when a plugin-using command runs in the project, then the global copy is used and reported with `"level": "global"`. When v2.9.3 is then installed into the project, the project copy is used and reported with `"level": "project"`.

**Binaries are ignored, declarations are committed**
Given a project root Rover has just created, when it writes the root, then it contains `rover.yaml`, `plugin-versions.lock`, `bin/`, and a `.gitignore` ignoring `bin/`. An existing `.rover/.gitignore` must be left untouched.

**A project root is self-contained**
Given a project with its plugins installed locally, when `<project>/.rover/bin/` is deleted and `rover plugin install` is re-run, then every plugin the lockfile records is restored, and the global install root is neither read from nor written to.

**Reproducible install from a committed lockfile**
Given a project with a committed `rover.yaml` declaring `supergraph: "2"` and a `plugin-versions.lock` recording `2.9.3`, when `rover plugin install` runs on two clean machines, then both install exactly `2.9.3` into their project roots, neither makes a resolution request to the registry, and `rover supergraph compose --format json` on both reports `"version": "2.9.3"`.

**Installing one plugin does not re-resolve the others**
Given a project lockfile recording `supergraph: 2.9.3` (requested `2`) and `router: 2.1.0` (requested `latest`), and newer releases of both available, when `rover plugin install router@latest` runs, then `router` is re-resolved and updated and `supergraph`'s locked `2.9.3` is untouched.

**Explicit resolution rewrites the lock**
Given the same project, when `rover plugin resolve` runs, then every floating alias is re-resolved, the plugins are installed into the project root, and `plugin-versions.lock` is rewritten with each `requested` and `resolved` pair.

**`--no-save`**
Given a project, when `rover plugin install router@2 --no-save` runs, then the binary is installed and the lockfile updated, and `rover.yaml` is not modified.

**Floating alias without a lock**
Given a `rover.yaml` declaring `supergraph: "2"` and no lockfile, when `rover supergraph compose` runs, then the alias is resolved against the registry, the newest 2.x is used, no lockfile is written, and the run prints `Using the supergraph plugin v<resolved> (downloaded).`

**Project overrides global, per plugin**
Given a global manifest declaring `supergraph: "2"` and `router: latest`, and a project manifest declaring only `router: "=2.1.0"`, when a plugin-using command runs inside that project, then `router` resolves to `2.1.0` and `supergraph` resolves from the global declaration — and installs into the project root regardless of which level supplied its version.

**Discovery walks up**
Given a project manifest at `<root>/.rover/rover.yaml`, when a command runs from `<root>/packages/a/b`, then the manifest is found and applied without any path flag.

**Home directory is not a project**
Given a working directory under `~` with no `.rover/` between it and `~`, when a plugin-using command runs, then no project is discovered, `~/.rover/` is treated as the global level only, and resolution and lookup are global.

**`supergraph.yaml` outranks the manifest**
Given `supergraph.yaml` with `federation_version: =2.9.3` and a manifest declaring `supergraph: "=2.8.0"`, when `rover supergraph compose` runs, then `2.9.3` is used and the FR30 warning is printed once.

**Flag outranks everything**
Given the same setup plus `--federation-version =2.7.0`, when `rover supergraph compose` runs, then `2.7.0` is used, and no lockfile entry applies to it.

**Offline guarantee, plugin present**
Given every declared plugin installed and the registry unreachable, when any plugin-using command runs with `--skip-update`, then it completes successfully with zero outbound connections and reports each plugin as `already installed`.

**Offline fail-fast, plugin absent from both levels**
Given `supergraph` v2.9.3 installed at neither level, when `rover supergraph compose --skip-update` runs, then it fails within one second, makes no outbound connection, prints the FR51 text naming the plugin and both directories searched, and exits with the never-download error code — which appears under `error.code` in `--format json`.

**`--no-download` on install**
Given `supergraph` v2.9.3 not installed, when `rover plugin install supergraph@=2.9.3 --no-download` runs, then it fails with the FR51 text naming `--no-download`, no artifact is downloaded, and neither the manifest nor the lockfile is modified.

**The two controls stay separate**
Given `APOLLO_ROVER_SKIP_UPDATE=true` and a plugin that is not installed, when `rover plugin install supergraph@=2.9.3` runs, then the plugin is downloaded and installed, because `--skip-update` does not guard the explicit install step.

**Idempotent install, per level**
Given `supergraph` v2.9.3 already in the project root, when `rover plugin install supergraph@=2.9.3` runs, then nothing is downloaded, `supergraph v2.9.3 is already installed in this project.` is printed, and the exit code is 0. When the same command runs with `--global` and the plugin is absent globally, then it is downloaded and installed globally, leaving the project copy alone.

**Uninstall removes the binary and the declaration**
Given `supergraph` v2.9.3 installed in the project and declared in its `rover.yaml`, when `rover plugin uninstall supergraph` runs, then the binary is removed from `<project>/.rover/bin/`, the `supergraph` entry is removed from both `rover.yaml` and `plugin-versions.lock`, and the global level is untouched.

**Uninstall does not reach across levels**
Given `supergraph` installed globally and not in the project, when `rover plugin uninstall supergraph` runs inside the project, then nothing is removed, the FR46 text naming `--global` is printed, and the global copy still satisfies a subsequent `rover supergraph compose` in that project.

**Uninstall version selection**
Given `supergraph` v2.9.2 and v2.9.3 both installed at the target level, when `rover plugin uninstall supergraph@=2.9.2` runs, then only v2.9.2 is removed. When `rover plugin uninstall supergraph` runs, then both are removed. When `rover plugin uninstall supergraph@2` runs, then it is rejected with the FR44 text and nothing is removed.

**Uninstall with `--no-save` warns about the drift it creates**
Given `supergraph` installed and declared, when `rover plugin uninstall supergraph --no-save` runs, then the binary and the lockfile entry are removed, `rover.yaml` still declares it, the FR45 warning is printed, and the next plugin-using command fails with the FR18 drift error.

**Uninstalling what isn't there**
Given `supergraph` installed at no level, when `rover plugin uninstall supergraph` runs, then nothing is removed, the command says so, and it exits 0.

**Batch install with one failure**
Given three declared plugins where the second's artifact is unavailable, when `rover plugin install` runs, then all three are attempted, the first and third are installed, the second's failure is reported with its error code, the command exits non-zero, and re-running installs only the second.

**Fallback provenance**
Given `supergraph` 2.9.2 installed and the registry unreachable, when `rover supergraph compose` runs without `--skip-update`, then composition succeeds using 2.9.2, both the existing staleness warning and `Using the supergraph plugin v2.9.2 (fallback: couldn't reach the plugin registry).` are printed, and `--format json` reports `"source": "fallback"`.

**Provenance in a long-running session**
Given `rover dev` running with `supergraph` v2.9.3, when a subgraph schema changes and recomposition occurs, then no new provenance line is printed. When `supergraph.yaml`'s `federation_version` changes to `=2.8.0` mid-session, then a new provenance line is printed for `2.8.0`.

**Unified grammar across plugins**
Given `rover plugin install router@2 supergraph@2 apollo-mcp-server@2`, when it runs, then all three requests parse identically as a bare major, and none is rejected for using a form another plugin requires.

**Deprecated forms still work**
Given `supergraph.yaml` with `federation_version: latest-2`, when `rover supergraph compose` runs, then it resolves exactly as `2` would, and the FR4 deprecation warning is printed once.

**Federation 1 is still rejected**
Given `federation_version: latest-1`, when `rover supergraph compose` runs, then it fails with the existing Federation 1 rejection, not a version-grammar error.

**Redirected install root**
Given a project manifest with `install_root: ../vendor/rover`, when `rover plugin install` runs in that project, then binaries are written under `<project>/../vendor/rover/bin/`, and a subsequent `rover supergraph compose` in that project finds them there.

**Lockfile drift**
Given a manifest declaring `router: "=2.2.0"` and a lockfile recording `2.1.0`, when a plugin-using command runs, then it fails with the FR18 text and does not silently re-resolve.

**A yanked version**
Given a project lockfile recording `supergraph: 2.9.3` and that release withdrawn from the registry, when `rover plugin install` runs, then it fails with the yanked-version error code — distinct from the resolution-failure code — names where the request came from, suggests the newest available 2.x by exact version, and installs nothing in its place.

**Checksum mismatch**
Given the registry publishes a checksum that does not match the served artifact, when `rover plugin install supergraph@=2.9.3` runs, then the install fails with the checksum error code, and no binary is left in the install root.

**A mirror supplies artifacts, not resolution**
Given a configured GitHub-release-layout mirror and a reachable registry, when `rover plugin install supergraph@2` runs, then the alias is resolved against the registry and the artifact is fetched from the mirror. Given the same mirror and an unreachable registry, when the same command runs, then it fails with the FR68 text; when `rover plugin install supergraph@=2.9.3` runs instead, then it succeeds entirely from the mirror.

**Global HTTP settings apply to resolution**
Given `--client-timeout 1` and a registry that stalls on the resolution request, when a plugin-using command runs, then it fails within the timeout rather than hanging.

**Listing across levels**
Given `supergraph` v2.9.3 installed globally and `router` v2.1.0 installed in the project, when `rover plugin list` runs inside that project, then both are listed with exact version, location, and level, and the copy each plugin-using command would use is marked. `--global` lists only the global level. `--format json` carries the same fields.

**No project means global, and nothing is invented**
Given no `.rover/` directory anywhere in scope, when `rover plugin install supergraph@=2.9.3` runs, then the plugin installs globally exactly as it does today, and no `.rover/` directory is created in the working directory. This holds at every release, including 1.0.

**Opting a project in**
Given no `.rover/` directory in scope, when `rover plugin install supergraph@=2.9.3 --manifest-path .rover/rover.yaml` runs, then `.rover/` is created in the working directory with `rover.yaml`, `plugin-versions.lock`, `bin/`, and a `.gitignore` ignoring `bin/`, and the plugin is installed there. Passing `--manifest-path` together with `--global` must be rejected before any network call.

**`--manifest-path` overrides discovery**
Given a project at `<root>/.rover/` and a second at `<root>/packages/b/.rover/`, when a plugin-using command runs from `<root>/packages/b` with `--manifest-path <root>/.rover/rover.yaml`, then `<root>`'s declarations and install root are used and `<root>/packages/b/.rover/` is ignored entirely.

**`--manifest-path` never creates outside `install`**
Given a path that does not exist, when `rover supergraph compose --manifest-path ./nope/rover.yaml` runs, then it fails with the malformed-declaration error code naming the path, and no directory or file is created.

**Unconfigured user is unaffected**
Given no manifest, no lockfile, and no new flags or environment variables, when any plugin-using command runs, then behavior is identical to today except that the FR54 provenance line is printed.

**Rover 1.0 opt-in download**
Given Rover 1.0, no `allow_automatic_download`, and `supergraph` v2.9.3 installed at neither level, when `rover supergraph compose` runs, then nothing is downloaded and the FR77 text is printed. Given `allow_automatic_download: true` in the project manifest, or `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD=true` in the environment, when the same command runs, then the plugin is downloaded as it is pre-1.0.

**A standing opt-in never overrides a per-invocation control**
Given Rover 1.0 and `allow_automatic_download: true`, when `rover supergraph compose --skip-update` runs and the plugin is installed at neither level, then nothing is downloaded and the command fails with the never-download error code.

---

## 5. Non-goals

- A general package manager: no inter-plugin dependencies, no third-party plugins, no marketplace. The supported set stays `supergraph`, `router`, `apollo-mcp-server`.
- Compiling any plugin into the Rover binary.
- Distributing plugins as npm packages, or bundling fallback binaries into the Rover install.
- A shared content-addressed store with links into project roots, pnpm-style. Each project root holds real binaries. Worth its own proposal if per-project duplication turns out to bite in practice; see §3.5.
- musl composition support, which depends on the `supergraph` binary shipping musl builds.
- Changing how the registry publishes aliases, or how Federation is released.
- Changing ELv2 license acceptance. A plugin that requires ELv2 acceptance today still requires it, including when installed from a manifest.
- Making plugin download opt-in before Rover 1.0.

---

## 6. Decisions

Decisions taken while drafting and their reasoning are provided here.

- **`supergraph.yaml` is authoritative for the `supergraph` plugin** (FR30), over the project manifest winning. A repository may hold several `supergraph.yaml` files on different Federation versions, and adding a `rover.yaml` must not silently change existing composition output.
- **The registry always resolves versions; a mirror only serves artifacts** (FR67), over requiring mirrors to expose a release listing. The registry can answer "newest release within major X" for every plugin, so nothing is gained by pushing that onto a mirror. The cost is stated in FR68: on a registry-less network, requests must already be exact.
- **`allow_automatic_download`, with `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD`** (FR78), over a manifest key alone and over the shorter `allow_download`. "Automatic" is the load-bearing word: an explicit `rover plugin install` is never gated by it.
- **Per-invocation controls outrank the standing opt-in** (FR79), over letting a manifest re-enable downloads a command was told not to make.
- **A `plugin` noun with `install`, `list`, `uninstall`, `resolve`** (FR33), over flags on `rover install`. `rover install --plugin` survives as a deprecated alias (FR34).
- **Project-local installs, global when no project is in scope** (FR23), over Cargo's global-first default and over npm's "invent a project in the working directory." Rover never creates a project root on its own (FR22), which is what keeps the change non-breaking (FR74).
- **Yanked versions get their own error code** (FR61), over folding them into general resolution failure, so a script can distinguish a version that never existed from one withdrawn.

---

## 7. Test obligations

This contract cannot be verified by unit tests alone. Four gaps must be closed alongside the work:

- **End-to-end coverage of the `dev` and `compose` plugin paths**, which does not exist today. Every acceptance criterion above that names a plugin-using command needs one.
- **Two-level fixtures.** Project-versus-global precedence, lookup order, and `rover plugin list` cannot be exercised without tests that control both a temporary project root and a temporary global level, on every supported platform.
- **A network-denied environment** in CI, since FR53's "zero outbound connections" and FR51's fail-fast are not observable in a test that merely points at an unreachable host.
- **Snapshot coverage of the JSON provenance envelope** (FR57) for each plugin-using command, so the `data.plugins` shape cannot regress unnoticed.
