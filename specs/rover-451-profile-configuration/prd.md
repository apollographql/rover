# PRD: Rover Profile Configuration

- **Owner:** Brian George
- **Component:** Rover CLI (profile management and configuration)

This PRD has a prerequisite and two parts that share a problem statement and a settings vocabulary but ship independently. The **Prerequisite** (flag / env var parity) gives every global setting both a flag and an env var, so the precedence rule Part A introduces has the same two upper tiers for every setting. **Part A** (profile-scoped settings) is complete on its own and fully addresses Problems 1 and 3 plus the individual-developer half of Problem 2. **Part B** (project-scoped settings) addresses the team/CI half of Problem 2 and depends on Part A. A spec for a Part A slice can be written from the Prerequisite and Part A alone.

## 1. Problem statement

Rover's configuration surface today is split between two mechanisms that don't compose:

- **Named profiles** (`--profile`, `rover config auth|list|delete|whoami`), which store exactly one thing per profile: a credential (API key or OAuth session).
- **Nearly two dozen `APOLLO_*` environment variables** (registry URL, telemetry opt-out/endpoint, checks timeout, VCS context overrides, download host, dev-command version pins, etc.), plus a handful of global flags with no env var (OAuth endpoints), which are process-global rather than profile-scoped, and must be re-set in every shell or CI job that wants non-default behavior.

This produces three concrete problems:

1. **Config sprawl / inconsistent precedence.** There is no single source of truth for "how is rover configured right now." Settings are scattered across env vars, CLI flags, and the profile credential. Precedence between them is inconsistent in practice (some flags beat their env var, some settings have only an env var and no flag) and the documentation's one blanket statement ("an environment variable's value takes precedence over all other methods") does not match how every setting actually behaves. Six of the env vars Rover reads are not documented at all.
2. **Non-secret configuration has no home.** Settings like registry URL, checks timeout, and telemetry opt-out are not sensitive, but the only way to persist them is to put them in every shell's rc file and in every CI pipeline definition that needs them. On a developer machine this means duplicating them per shell and per project; in CI it means the pipeline definition is the config store, so a team can't share "how our repo talks to GraphOS" without also sharing infrastructure config.
3. **Multi-environment / multi-org workflows.** A user working across multiple GraphOS orgs or a non-default registry can already switch credentials via `--profile`, but everything else stays global. Switching profiles today only switches the credential, not the rest of the "environment." A profile for a staging org can't even refresh its own OAuth token against staging, because the OAuth endpoints are global flags.

## 2. Background: how this works today

- **Profiles store only a credential.** A profile holds an API key or an OAuth session. The secret is kept in the OS keychain, with a legacy plaintext fallback for older installs. No other setting is profile-scoped.
- **Precedence already exists for the one thing profiles store.** When resolving a credential, Rover prefers the `APOLLO_KEY` env var, then an OAuth client-credentials exchange (`APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET`), then the stored profile credential. An env var beats even an explicitly selected `--profile`'s stored credential. This is deliberate: CI needs a zero-setup override that wins regardless of what's on disk.
- **Everything else is env-var/CLI-flag only.** The full set of environment variables Rover reads today:

  | Env var | Purpose | Documented today |
  |---|---|---|
  | `APOLLO_KEY` | API key override; wins over any stored profile credential | Yes |
  | `APOLLO_CLIENT_ID` / `APOLLO_CLIENT_SECRET` | OAuth client-credentials grant (CI / machine-to-machine auth) | Yes |
  | `APOLLO_CONFIG_HOME` | Location of Rover's config directory, where profiles live | Yes |
  | `APOLLO_HOME` | Location where Rover installs its binary and plugins | Yes |
  | `APOLLO_REGISTRY_URL` | Overrides the GraphOS registry endpoint | No |
  | `APOLLO_TELEMETRY_DISABLED` | Opts out of anonymous usage telemetry | Yes |
  | `APOLLO_TELEMETRY_URL` | Overrides the telemetry collection endpoint | No |
  | `APOLLO_CHECKS_TIMEOUT_SECONDS` | Overrides how long check/launch polling waits before giving up | No |
  | `APOLLO_VCS_REMOTE_URL` / `APOLLO_VCS_BRANCH` / `APOLLO_VCS_COMMIT` / `APOLLO_VCS_AUTHOR` | Override the Git context sent to GraphOS on check/publish | Yes |
  | `APOLLO_GRAPH_REF` | Forwarded to the router that `rover dev` spawns, to enable GraphOS Router features. Not used by Rover itself for schema retrieval; that is what the `--graph-ref` option does. The two are complementary, not alternatives. | Yes |
  | `APOLLO_ELV2_LICENSE` | Accepts the ELv2 license non-interactively | Yes |
  | `APOLLO_ROVER_SKIP_UPDATE` | Opts out of self-update and plugin auto-update checks | No |
  | `APOLLO_ROVER_DOWNLOAD_HOST` | Overrides the host plugins (router, composition) are downloaded from | Yes |
  | `APOLLO_ROVER_DEV_MCP_VERSION` / `APOLLO_ROVER_DEV_ROUTER_VERSION` / `APOLLO_ROVER_DEV_COMPOSITION_VERSION` | Pin binary versions used by `rover dev` | Yes |
  | `APOLLO_NODE_MODULES_BIN_DIR` | Adjusts install/lookup paths when Rover is invoked via the npm wrapper | Yes |
  | `APOLLO_TEMPLATES_API` | Overrides where `rover init` fetches project templates | No |
  | `APOLLO_NO_COLOR` / `NO_COLOR` | Disables colored output | No |

  None of these are persisted anywhere; they're re-read from the process environment on every invocation.
- **OAuth endpoints are global flags with no env var.** `--oauth-authorization-url`, `--oauth-token-url`, `--oauth-device-authorization-url`, `--oauth-revocation-url`, `--oauth-whoami-url`, and `--oauth-client-id` each have a built-in default pointing at production GraphOS. They are network destinations that receive or mint credentials, and they are the reason a non-production profile can't be self-contained today.
- **Boolean env vars don't share a convention.** `APOLLO_TELEMETRY_DISABLED` is presence-only: any value, including `false`, disables telemetry. `APOLLO_NO_COLOR` is a mirror of the cross-tool `NO_COLOR` variable and shares its parser, which is a deny-list: empty, `0`, and `false` count as unset, anything else as set. That divergence comes from tracking `NO_COLOR`, not from a Rover decision, and is not something to normalize. `APOLLO_ROVER_SKIP_UPDATE` is the opposite shape, an allow-list: only `1` or `true` (case-insensitive) counts as set, anything else as unset. (`APOLLO_ELV2_LICENSE` is not a boolean at all; it accepts only the literal `accept`.) Any persisted representation of these settings needs one typed boolean, not three string conventions.
- **Flag vs. env precedence differs per setting.** Where a setting has both a flag and an env var (e.g. `--elv2-license` / `APOLLO_ELV2_LICENSE`), the flag wins. Some settings have only an env var (`APOLLO_CHECKS_TIMEOUT_SECONDS`), some only a flag (`--client-timeout`, the OAuth endpoints). Most of the settings this PRD targets have no flag at all. The docs' blanket "env var wins over all other methods" is not accurate today.
- **`--profile` is per-command and has a built-in default.** Only commands that talk to GraphOS accept `--profile`, and omitting it is indistinguishable from passing `--profile default`. Commands that don't talk to GraphOS (local composition, introspection against a URL, plugin install, docs, completions) never read a profile today. Telemetry is resolved after the full command line is parsed but before any subcommand runs, and today writes a machine-identifier file into the config directory.
- **There is no unified view of configuration.** The credential and the settings above are resolved independently, and nothing today reports where a given effective value came from.

## 3. Shared definitions

- **Setting.** A named, typed, non-secret configuration value. Settings are named by their env var name verbatim (`APOLLO_REGISTRY_URL`) in `rover config`, in the project file, and in the inspection verb. Settings that have no env var today (the OAuth endpoints) gain an `APOLLO_OAUTH_*` env var in the Prerequisite slice, so by the time Part A ships every setting has an env var name.
- **Network-destination setting.** A setting whose value is a host or URL that Rover sends requests to or downloads code from: `APOLLO_REGISTRY_URL`, `APOLLO_TELEMETRY_URL`, `APOLLO_ROVER_DOWNLOAD_HOST`, `APOLLO_TEMPLATES_API`, and the OAuth endpoint settings.
- **Explicitly selected profile.** A profile named by `--profile <name>` on the command line, including `--profile default` typed literally. Omitting `--profile` selects the default profile implicitly. There is deliberately no env var for this (P2).
- **Profile-eligible.** A setting belongs in a profile if switching GraphOS org or environment would plausibly require changing it. Settings that don't vary by org (update-check opt-out, npm wrapper paths, color output, ELv2 acceptance) are not eligible.
- **Override.** A higher-precedence source supplied a value for a setting and a lower-precedence source also supplied one. Determined by presence, not by comparing values.

---

# Prerequisite: flag / env var parity for global configuration

Ships before Part A slice one, as its own slice. It stands alone: even without profiles, a user should be able to configure anything process-global from either the command line or the environment.

## P1. Goals

1. **Every process-global setting has both a flag and an env var.** A setting is process-global if Rover resolves it once, from the top-level command line or the environment, and applies it to whichever command runs. Today roughly half of these have only one of the two.
2. **One precedence rule.** For every pair: **explicit CLI flag > environment variable > built-in default**. This is the top two tiers of A1.3; Part A inserts the profile tier beneath the env var without changing these.
3. **One boolean convention for env vars.** A boolean env var is set when its value is `1` or `true` (case-insensitive) and unset when it is `0`, `false`, empty, or absent. Every env var added by this slice follows it. Existing boolean vars keep their current parsing unless A4 decides otherwise, so nothing that works today stops working.
4. **Every pair is documented together.** The supported-environment-variables table in the docs lists the flag beside each env var, and the flag's help text names the env var, so a reader finds the other half from either side.
5. **Full backward compatibility.** No existing flag or env var changes meaning, default, or parsing. Everything added is additive.

## P2. Pairings

Names below are the proposal; the spec confirms them.

**Env-only settings that gain a flag:**

| Env var | Flag | Scope |
|---|---|---|
| `APOLLO_REGISTRY_URL` | `--registry-url` | global |
| `APOLLO_TELEMETRY_URL` | `--telemetry-url` | global |
| `APOLLO_TELEMETRY_DISABLED` | `--telemetry-disabled` | global |
| `APOLLO_CHECKS_TIMEOUT_SECONDS` | `--checks-timeout` | global |
| `APOLLO_VCS_REMOTE_URL` / `_BRANCH` / `_COMMIT` / `_AUTHOR` | `--vcs-remote-url`, `--vcs-branch`, `--vcs-commit`, `--vcs-author` | global |
| `APOLLO_ROVER_DOWNLOAD_HOST` | `--download-host` | global |
| `APOLLO_NO_COLOR` | `--no-color` | global |
| `APOLLO_CONFIG_HOME` | `--config-home` | global |
| `APOLLO_HOME` | `--rover-home` | global |
| `APOLLO_ROVER_DEV_ROUTER_VERSION` / `_COMPOSITION_VERSION` | `--router-version`, `--composition-version` | `rover dev` only, matching the existing `--mcp-version` pair |
| `APOLLO_TEMPLATES_API` | `--templates-api` | `rover init` and `rover template` only |

**Flag-only settings that gain an env var:**

| Flag | Env var |
|---|---|
| `--oauth-authorization-url` | `APOLLO_OAUTH_AUTHORIZATION_URL` |
| `--oauth-token-url` | `APOLLO_OAUTH_TOKEN_URL` |
| `--oauth-whoami-url` | `APOLLO_OAUTH_WHOAMI_URL` |
| `--oauth-revocation-url` | `APOLLO_OAUTH_REVOCATION_URL` |
| `--oauth-device-authorization-url` | `APOLLO_OAUTH_DEVICE_AUTHORIZATION_URL` |
| `--oauth-client-id` | `APOLLO_OAUTH_CLIENT_ID` |
| `--log` | `APOLLO_LOG_LEVEL` |
| `--format` | `APOLLO_FORMAT` |
| `--client-timeout` | `APOLLO_CLIENT_TIMEOUT` |

**Deliberately unpaired:**

| Setting | Why |
|---|---|
| `APOLLO_KEY`, `APOLLO_CLIENT_ID`, `APOLLO_CLIENT_SECRET` | Credentials. A flag would put the secret in shell history and the process list; `rover config auth` exists precisely to avoid that. |
| `--output` | A file path for this invocation's output has no sensible environment-level default. |
| `--profile` | Becomes a global flag (P3) but gets no env var. Which profile a command runs against should be visible on the command line, not inherited from the shell; an env var would make "which org did that just hit" depend on ambient state. A container that always wants one profile passes it in its entrypoint. |
| `--insecure-accept-invalid-certs`, `--insecure-accept-invalid-hostnames` | An env var would make an insecure TLS setting invisible and persistent across every command in a shell. Keeping these flag-only means every insecure invocation is visible where it is typed. A CI image that needs them for every invocation passes them in its entrypoint. |
| `APOLLO_ROVER_SKIP_UPDATE` | Already covered by the union of `--skip-update-check` (global, self-update) and `--skip-update` (per command, plugin auto-update). No new flag; the docs state the relationship. |
| `APOLLO_ELV2_LICENSE`, `APOLLO_ROVER_DEV_MCP_VERSION` | Already paired with `--elv2-license` and `--mcp-version`. Listed so the audit is complete. |
| `APOLLO_GRAPH_REF` | Already has a flag counterpart, but with different semantics (§2). Left as-is pending the A4 graph-ref question. |
| `APOLLO_NODE_MODULES_BIN_DIR` | Internal, set by the npm installer. Not part of the public contract. |

## P3. Key decisions and rationale

- **`--profile` becomes a global flag.** Today it exists only on commands that talk to GraphOS, so telemetry and commands like local composition cannot see it. Part A needs one active-profile resolution before any command runs (A2); a global flag is how a CLI expresses that. Commands that never read a profile accept the flag and ignore it, which is already how the other global flags behave.
- **Bootstrap settings get flags but not profile storage.** `APOLLO_CONFIG_HOME` and `APOLLO_HOME` gain flags for parity, but they remain excluded from profiles (A3) because they determine where profile data lives.
- **Parity is the default, not an absolute.** Two flags stay flag-only on purpose (`--profile` and the two insecure TLS flags, see P2). In both cases the setting changes where requests go or how they are trusted, and a value inherited from the shell would make that invisible at the point of use. Containers and CI images that want them on every invocation put them in the entrypoint, which keeps them visible in the image definition.
- **New boolean env vars share one convention; old ones are not silently changed.** Rover has three boolean conventions today (§2). Adding a fourth would compound the problem, so every new var uses the `1`/`true` rule. Changing existing vars to match is a behavior change and is left to A4.
- **No shared "default flag" behavior is inferred.** Where a flag has a built-in default today (the OAuth endpoints, `--format`, `--client-timeout`), adding an env var must not change what happens when neither is supplied.

## P4. Success metrics

- For every pair in P2, setting the value through the flag alone, the env var alone, and both at once produces flag-wins, env-applies, and flag-wins respectively, verified by the inspection verb once Part A lands and by behavior until then.
- `rover --help` lists every global flag in P2, and each flag's help text names its env var.
- The supported-environment-variables docs table lists every env var in P2 with its flag, and every env var the §2 table marks undocumented is documented.
- Rover's existing end-to-end suite passes unchanged.

---

# Part A: Profile-scoped settings

## A1. Goals

1. **Profile-scoped settings.** A named profile can carry any profile-eligible setting in addition to its credential (candidate list in A4).
2. **Full backward compatibility.** Every existing `APOLLO_*` environment variable and every existing flag continues to work exactly as it does today. No deprecation. Profile values are an *additional* source, not a replacement. A user who never creates a profile setting sees zero behavior change, including no new files or directories being created beyond what Rover creates today.
3. **One precedence rule for settings.**

   **explicit CLI flag > environment variable > profile > built-in default**

   "Explicit flag" means a flag actually passed on the command line; a flag's built-in default is the built-in-default tier. This rule covers settings only; credentials keep their existing resolution chain (§2). Precedence is about presence: if an env var is present it wins, and whether Rover then accepts or rejects its value follows that setting's behavior today. The docs' current precedence statement is corrected to match.
4. **Legible effective configuration.** A new `rover config` inspection verb (name settled in the spec) reports, per setting, its effective value and which source it came from, in both human-readable text and `--format json`. Credential values are always masked. The inspection verb does not itself emit A1.5 notices.
5. **Overrides are visible when they matter.** Rover prints a one-line notice to stderr when, for a setting the current command uses, either (a) an env var overrides a value that an explicitly selected profile also supplies, or (b) the profile supplies a non-default value for a network-destination setting. "Uses" means the command issues a request to that destination during this invocation. At most one notice per setting per process, on first use, combining both reasons if both apply. Notices never appear inside the JSON envelope, are not suppressed by `--format json`, never affect the exit code, and can be suppressed only by a non-persisted source (env var or flag, named in the spec), never by anything stored on disk.
6. **`rover config` covers settings, not just credentials.** Users can set, read, and unset individual settings on a profile through `rover config`, with values validated syntactically at write time so a malformed URL or non-numeric timeout is rejected when entered. Settings and (in Part B) trust verbs live under `rover config`; existing credential verbs (`rover config auth`, `rover auth login|logout`) are unchanged and no new credential verbs are added.

## A2. Key decisions and rationale

- **Additive, no deprecation.** Env vars are the only zero-setup configuration path: CI images, ephemeral containers, and one-off scripts can't run interactive `rover config` commands. Removing or deprecating env vars would break the path most automation depends on.
- **Env var beats an explicitly selected profile.** This generalizes the existing `APOLLO_KEY` rule: CI must be able to override anything on disk without editing it. The footgun (`APOLLO_REGISTRY_URL=staging rover graph check --profile prod` uses staging's registry with prod's key) is real, so A1.5(a) surfaces it rather than leaving it silent.
- **`--profile` must be distinguishable from its default.** A1.5(a) and Part B both depend on knowing whether the user passed `--profile`. Today they can't be told apart. The Prerequisite slice delivers this by making `--profile` a global flag, resolved once from the parsed command line before any command runs; Part A consumes that distinction rather than re-establishing it.
- **OAuth endpoints are in scope.** A staging-org profile that can't refresh its own token against staging isn't a profile, it's a credential with extra steps. Bringing the six OAuth endpoint flags under the same settings model is what makes per-profile OAuth sessions coherent, and they are network destinations, so they get A1.5(b) surfacing. Because they have defaults today, the "explicit flag" rule is what lets a profile value take effect.
- **Non-secret settings are stored in plaintext, not the keychain.** Only credentials belong in the OS keychain. Putting registry URLs or timeouts there would trigger keychain prompts on commands that have never touched the keychain and would make settings unreadable by simple tooling. The tamper risk of a plaintext settings file in the user's home is addressed by A1.5's surfacing; an attacker with write access to the user's config directory is outside this PRD's threat model.
- **Settings live with the profile.** A profile's settings are stored alongside its credential, so `rover config delete <name>` and `rover config clear` remove both. Re-authenticating (`rover config auth`) and logging out (`rover auth logout`) preserve settings; only the credential changes.
- **Notice suppression only from non-persisted sources.** A "quiet this notice" key in the same file an attacker would tamper with defeats the surfacing. Orgs with a mirrored registry already put that URL in shell and CI setup; the suppression goes in the same place.
- **The active profile is resolved once, from the parsed command line, before any command runs.** Telemetry and every command, including those without a `--profile` option today (local composition, plugin install, docs), use that single resolution, so telemetry honors an explicitly selected profile like everything else. Commands with no `--profile` option use the default profile. Reading settings must never create the config directory or any file beyond what Rover creates today; a read-only environment with no config works exactly as it does now.
- **Missing profile is not an error.** If the active profile has no stored settings, or doesn't exist on disk at all, every setting falls through to the next source. A CI job with only `APOLLO_KEY` and no config directory must work exactly as today.
- **`rover config` can create a settings-only profile.** Setting a value on a profile that doesn't exist yet creates it with no credential. A command that later needs a credential from that profile fails with a message saying the profile has no credential configured.
- **Unknown settings warn; invalid values fail.** A profile carrying a setting key the running Rover version doesn't recognize produces a stderr warning and is otherwise ignored, so sharing a config directory across Rover versions doesn't break the older one. A *known* key whose stored value fails syntactic validation fails the command with a message naming the key. Rover never checks reachability.
- **Empty env var means set.** An env var set to the empty string overrides lower sources with the empty value, preserving today's behavior per A1.2, and counts as an override for A1.5. The inspection verb makes this visible, which is the actual fix for the confusion it causes.
- **Child-process forwarding follows the env var.** Where Rover forwards an env var to a spawned process today (`APOLLO_KEY` and `APOLLO_GRAPH_REF` to the router in `rover dev`), the same setting resolved from a profile is forwarded identically. Settings Rover doesn't forward today aren't forwarded from a profile either.

## A3. Non-goals

- Deprecating or removing any existing `APOLLO_*` environment variable or flag.
- Moving `APOLLO_CONFIG_HOME` or `APOLLO_HOME` into profile storage. Both are bootstrap-time (they determine where profile data itself lives, or where Rover installs itself), so they can't depend on a profile being loaded.
- Changing the order in which the API key, OAuth client credentials, and stored profile credential are tried when resolving a credential.
- A global (non-profile) user settings file for machine-level preferences. Settings that fail the eligibility test are simply out of scope, not moved somewhere else.
- The on-disk format of the profile settings file.

## A4. Open questions (resolve in the Part A specs)

- **Candidate list.** Applying the eligibility test: `APOLLO_REGISTRY_URL`, `APOLLO_TELEMETRY_URL`, `APOLLO_TELEMETRY_DISABLED`, `APOLLO_CHECKS_TIMEOUT_SECONDS`, `APOLLO_VCS_*`, `APOLLO_GRAPH_REF`, `APOLLO_ROVER_DOWNLOAD_HOST`, `APOLLO_TEMPLATES_API`, and the six OAuth endpoints look eligible. `APOLLO_ROVER_SKIP_UPDATE`, `APOLLO_NODE_MODULES_BIN_DIR`, `APOLLO_ROVER_DEV_*_VERSION`, `APOLLO_ELV2_LICENSE`, and `APOLLO_NO_COLOR` look ineligible. The spec for each slice confirms its own subset.
- **Graph ref semantics.** `APOLLO_GRAPH_REF` and `--graph-ref` do different things (§2). A profile-stored graph ref must specify which behavior it drives: router-feature enablement only (matching the env var), schema retrieval (matching the flag), or both.
- **OAuth client credentials.** `APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET` are conceptually a credential, not a setting. Whether they become a third persisted credential kind in the profile is a credential-model question and may warrant its own PRD.
- **Boolean normalization for `APOLLO_TELEMETRY_DISABLED`.** Today it is presence-only, so `APOLLO_TELEMETRY_DISABLED=false` disables telemetry. Under the Prerequisite's shared boolean convention that value would mean "enabled". Decide whether to normalize it (a behavior change for anyone relying on the quirk) or keep it presence-only and document the exception; the profile-stored form is a typed boolean either way.
- **Delivery slicing.** Slice one: settings storage, `--profile` distinguishability, the inspection verb, notices, and the registry/telemetry/OAuth-endpoint group. Later slices: VCS context, graph ref, checks timeout, download host, templates API.

## A5. Success metrics

- **Problem 1.** Every setting listed in §2, including the six currently undocumented env vars and the OAuth endpoints, is documented with the single A1.3 precedence statement replacing the current blanket "env var wins" sentence. Running the inspection verb with no configuration present reports every setting as built-in default.
- **Problem 3.** A user with two profiles pointing at two different registries and OAuth environments can log in to each and run the same GraphOS command against each by changing only `--profile`, with no env vars or flags set.
- **Compatibility.** Rover's existing end-to-end test suite passes unchanged once profile settings ship, when run with the config directory pointed at an empty location.
- **Legibility.** For every eligible setting and every source that setting supports, setting it via that source and running the inspection verb reports the correct source, and no credential value appears in the output.
- **Surfacing.** A command that issues a request to a non-default network destination supplied by a profile emits exactly one A1.5 notice on stderr; a command that doesn't emits nothing; setting the suppression env var silences it; the exit code is unaffected either way.

---

# Part B: Project-scoped settings

Depends on Part A. Introduces a committable artifact and a trust gate, and extends the precedence rule.

## B1. Goals

1. **Project-scoped settings.** A repository can carry a committable, non-secret configuration file that supplies any profile-eligible setting for anyone running Rover in that working tree. The repo, not the pipeline definition or each developer's shell, says how it talks to GraphOS. A credential key in the project file is an error, not a warning. Project files are hand-edited; there is no `rover config` verb that writes them.
2. **Extended precedence.** Part A's rule becomes:

   **explicit CLI flag > environment variable > explicitly selected profile > project file > default profile > built-in default**

   All other Part A rules (presence-based, settings only, explicit flag) carry over unchanged.
3. **Project files are trusted before they can redirect.** The first time Rover encounters a set of network-destination values from a project file it hasn't seen before, it shows each network-destination key and its full value and asks the user to trust them before applying them. Trust is recorded per user and keyed on the set of network-destination key/value pairs, regardless of which repository or path they appear in; any addition, removal, or change to that set re-prompts. Answering yes persists; answering no applies to this run only. Non-interactive runs can grant or refuse trust through an env var named in the spec; absent that, network-destination settings from an untrusted project file are ignored with a stderr warning naming them, and every other setting in the file still applies. Inert settings never require trust. The prompt is a gate, not a notice: it fires regardless of `--format json`.
4. **Trust is inspectable and revocable.** A `rover config` verb lists trusted value sets and revokes them. The inspection verb reports a network-destination setting that was present in the project file but ignored for lack of trust, with that as its source.
5. **Surfacing extends to project files.** A1.5(b) also fires when the project file supplies a non-default network-destination value.

## B2. Key decisions and rationale

- **Explicitly selected profile beats the project file.** When a user types `--profile staging`, they have named the environment they want for this invocation. Letting a file in the working tree silently override that reproduces the env-var footgun with no CI justification to excuse it. `--profile default` typed literally is explicit, which is the user's escape from a repo's settings. The implicit default profile sits *below* the project file because omitting `--profile` expresses no intent, and a team's committed settings should win over a user's unstated default.
- **The project file cannot select a profile.** Considered and rejected: it would need its own precedence tier, would silently no-op when the named profile doesn't exist on the user's machine, and would let a repo's own file override the repo's own settings depending on which tier it landed in. Users targeting one org from one repo pass `--profile` or make it their default.
- **Network destinations require trust.** A project file arrives via `git clone`. Without a gate, cloning a repo and running local composition would honor a download host that serves a trojaned binary, and running a check would send the user's API key to whatever registry URL the repo names. A notice printed at request time is a receipt, not a gate. Making network-destination settings ineligible for project files was rejected because "our repo uses our mirrored registry" is a primary reason teams want a project file. A profile-level host allowlist was rejected as a two-step setup nobody would discover. The trust prompt is the model editors use for workspace trust and is the least setup for the common case.
- **Trust is keyed on values, not location.** The threat is the destination, not the directory it was declared in. Keying on path would re-prompt for every git worktree, renamed checkout, and fresh CI workspace, forcing CI to set the trust env var permanently and making the gate a no-op there. Two repos that point at the same mirror legitimately share trust. Any change to the value set re-prompts, so a later commit can't swap the registry under an already-trusted user.
- **Untrusted means ignored, not fatal.** A CI job that hasn't opted in should not have its API key sent to a repo-specified host, and should not fail outright over a setting it may not need. This is deliberately asymmetric with "invalid values fail": an untrusted value is a user's choice, an invalid value is a bug in the file. A developer who answers "no" proceeds against the profile's or default destination with a warning saying so.
- **The trust store shares the settings file's threat model.** It's a plaintext file in the user's config directory. An attacker with local write access could pre-trust a value set, exactly as they could edit a profile setting. That attacker is out of scope for this PRD; the trust gate defends against the remote attacker who controls a repository, not the local one who controls the home directory.
- **Honest CI accounting.** For a CI pipeline that today sets several `APOLLO_*` env vars, the project file replaces them with one: the trust env var. That is a real reduction but not zero; the pipeline still opts in.
- **Non-interactive detection.** A run is interactive if and only if both standard input and standard error are attached to a terminal. Everything else (CI, pre-commit hooks, process managers, piped invocations) is non-interactive and follows the env var rule.
- **Discovery.** Rover looks for the project file in the current directory, then each ancestor, stopping at the first file found, at a repository root, or at the filesystem root. The nearest file wins; files are never merged. Commands that take a configuration file in another directory still discover from the current directory.
- **Unknown and invalid values.** Same as Part A: unknown keys warn, invalid values for known keys fail the command with a message naming the file and key. A committed typo breaks loudly with a clear repair path rather than silently changing behavior.
- **Child-process forwarding.** Same as Part A: a setting resolved from a project file is forwarded to spawned processes exactly where its env var is today.

## B3. Non-goals

- Storing credentials of any kind in the project file.
- A project file selecting a profile (see B2).
- Merging multiple project files.
- The name and format of the project file, with one constraint: it must not live under a path Apollo tooling already tells users to ignore (`.apollo/`), or "committable" is defeated by common ignore templates.

## B4. Open questions (resolve in the Part B spec)

- **Discovery corner cases.** Nested project files in a monorepo (nearest wins is the rule; the spec confirms behavior for commands run from a subdirectory with a file at the root and another in the subdirectory), symlinked directories, and git worktrees.
- **Trust prompt answers.** Whether a "this run only" answer is offered in addition to yes/no.
- **Refusal in CI.** Whether the trust env var distinguishes "refuse" from "unset," or whether unset simply means refuse.

## B5. Success metrics

- **Problem 2.** A repo containing a project file (trusted) and no other configuration produces identical value/source pairs for every eligible setting for two different users on two different machines, verified by the inspection verb's JSON output.
- **Compatibility.** Rover's existing end-to-end test suite passes unchanged once project settings ship, when run with the config directory pointed at an empty location and no project file in any ancestor of the working directory.
- **Trust.** In a fresh clone whose project file sets a download host, running local composition non-interactively with no trust env var does not contact that host, warns that the setting was ignored, and otherwise succeeds. Running it interactively shows the host in the prompt; answering yes persists trust; the same value set in a second clone at a different path does not re-prompt.
- **Explicit profile wins.** In a repo whose project file sets a registry URL, `rover graph check --profile staging` uses staging's registry URL when staging defines one, with no notice.

---

## Rollout

- **Prerequisite slice.** Flag / env var parity for every global setting (P1 through P4), `--profile` as a global flag, the shared boolean env convention, and documentation of every pair. Ships before Part A slice one. It is a user-visible change in its own right and stands alone even if Part A were never built.
- **Slice one (Part A).** Settings storage alongside the profile credential, the inspection verb, the notices and their suppression, and the registry/telemetry/OAuth-endpoint setting group. With the Prerequisite in place, this slice only adds the profile tier between env var and built-in default. Includes the docs correction to the precedence statement, since the current statement is already inaccurate.
- **Later Part A slices.** VCS context, graph ref (after its semantics question is settled), checks timeout, download host, templates API. Each confirms its own eligibility subset.
- **Part B.** Its own spec and its own slice, after Part A's settings storage has shipped. Introduces the committable file, discovery, two precedence tiers, the trust prompt, trust storage, and the trust verbs together; they don't decompose usefully.
- Each shipped slice is a user-visible change and gets a changelog entry.
