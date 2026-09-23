# Spec: Rover profile and project configuration

- **PRD:** [prd.md](./prd.md)

## 1. Purpose and scope

This spec defines the observable contract for how Rover names, resolves, stores, reports, and refuses non-secret configuration. It covers every requirement in the PRD: the Prerequisite (P1–P4), Part A (A1–A5), and Part B (B1–B5).

It describes what Rover must do under which conditions, and the exact user-facing surface involved: setting names, flags, environment variables, file formats, message text, JSON fields, and error classes. It does not describe how any of it is built; implementation approach belongs in a separate implementation plan, one per slice.

Two things are deliberately outside it. **Credentials** keep the resolution chain they have today — API key environment variable, then an OAuth client-credentials exchange, then the stored profile credential — and nothing here changes it. **Discovery of the project manifest** belongs to the plugin system (ROVER-420); this spec adds a section to that file and inherits its discovery rule rather than defining a second one.

One observation motivates the whole Prerequisite and is worth stating up front, because it explains why a rule as obvious as "a flag beats its environment variable" needs specifying at all: today almost every environment variable Rover reads is read out-of-band rather than bound to the flag it corresponds to. Precedence between a flag and its variable is therefore a per-setting accident, and the published blanket rule does not describe any single one of them reliably.

Requirements are tagged with the PRD requirement they realize, e.g. `(A1.3)`.

## 2. Terminology

- **Setting**: a named, typed, non-secret configuration value. Every setting is named by its environment variable, verbatim and in full — `APOLLO_REGISTRY_URL`, never `registry_url` or `registry-url`. That spelling is the setting's **canonical name**.
- **Canonical name**: the spelling above. It is the name Rover prints (FR2) and the only name the verbs of §3.7 accept. The project file additionally accepts its all-lowercase form (FR69); nothing else does.
- **Credential**: an API key or an OAuth session. A credential is not a setting. No rule in this spec applies to one.
- **Source**: where an effective value came from. Exactly one of six: a command-line flag, the environment, an explicitly selected profile, the project file, the default profile, or Rover's built-in default.
- **Explicitly selected profile**: a profile named by `--profile <name>` on the command line, including `--profile default` typed literally.
- **Default profile**: the profile named `default`, selected because `--profile` was not passed.
- **Active profile**: whichever of the two the invocation resolved to.
- **Project file**: the `settings:` section of Rover's project manifest, `.rover/rover.yaml`. The manifest's other sections are not settings and no rule in this spec applies to them.
- **Network-destination setting**: a setting whose value is a host or URL Rover sends requests to or downloads code from. Marked in §3.1.
- **Bootstrap setting**: a setting that determines where Rover's own configuration or installation lives, and therefore cannot be read from a profile or a project file without circularity.
- **Profile-eligible** / **project-eligible**: a setting this spec permits to be stored in a profile / in the project file. §3.1 is the authority; §6 records why the two lists are not identical.
- **Override**: a higher-precedence source supplied a value for a setting and a lower-precedence source also supplied one. Determined by presence, never by comparing values.
- **Uses a setting**: the invocation actually acts on the setting's value — for a network-destination setting, it issues at least one request to that destination during the run.

---

## 3. Functional requirements

### 3.1 The settings catalogue (P1.1, P1.2, A4)

- **FR1**: The following table is the complete catalogue of Rover's settings. Every other requirement in this spec refers to it. A setting not in this table is not a setting, and none of the storage, precedence, inspection, or notice rules apply to it.

  | Setting | Flag | Scope of the flag | Type | Built-in default | Profile | Project | Net |
  |---|---|---|---|---|---|---|---|
  | `APOLLO_REGISTRY_URL` | `--registry-url` | global | URL | `https://api.apollographql.com/graphql` | ● | ● | ● |
  | `APOLLO_TELEMETRY_URL` | `--telemetry-url` | global | URL | `https://rover.apollo.dev/telemetry` | ● | ● | ● |
  | `APOLLO_TELEMETRY_DISABLED` | `--telemetry-disabled` | global | boolean | off | ● | ● | |
  | `APOLLO_CHECKS_TIMEOUT_SECONDS` | `--checks-timeout` | global | whole seconds | `300` | ● | ● | |
  | `APOLLO_CLIENT_TIMEOUT` | `--client-timeout` | global | whole seconds | `30` | ● | ● | |
  | `APOLLO_ROVER_DOWNLOAD_HOST` | `--download-host` | global | URL | `https://rover.apollo.dev` | ● | ● | ● |
  | `APOLLO_TEMPLATES_API` | `--templates-api` | `init`, `template` | URL | `https://rover.apollo.dev/templates` | ● | ● | ● |
  | `APOLLO_GRAPH_REF` | — (see FR6) | — | graph ref | none | ● | ● | |
  | `APOLLO_OAUTH_AUTHORIZATION_URL` | `--oauth-authorization-url` | global | URL | Apollo production | ● | ● | ● |
  | `APOLLO_OAUTH_TOKEN_URL` | `--oauth-token-url` | global | URL | Apollo production | ● | ● | ● |
  | `APOLLO_OAUTH_DEVICE_AUTHORIZATION_URL` | `--oauth-device-authorization-url` | global | URL | Apollo production | ● | ● | ● |
  | `APOLLO_OAUTH_REVOCATION_URL` | `--oauth-revocation-url` | global | URL | Apollo production | ● | ● | ● |
  | `APOLLO_OAUTH_WHOAMI_URL` | `--oauth-whoami-url` | global | URL | Apollo production | ● | ● | ● |
  | `APOLLO_OAUTH_CLIENT_ID` | `--oauth-client-id` | global | string | Apollo production | ● | ● | |
  | `APOLLO_LOG_LEVEL` | `--log` | global | log level | none | | | |
  | `APOLLO_FORMAT` | `--format` | global | `plain` \| `json` | `plain` | | | |
  | `APOLLO_NO_COLOR` | `--no-color` | global | boolean | off | | | |
  | `APOLLO_CONFIG_HOME` | `--config-home` | global | path | per platform | | | |
  | `APOLLO_HOME` | `--rover-home` | global | path | per platform | | | |
  | `APOLLO_VCS_REMOTE_URL` | `--vcs-remote-url` | global | string | inferred from Git | | | |
  | `APOLLO_VCS_BRANCH` | `--vcs-branch` | global | string | inferred from Git | | | |
  | `APOLLO_VCS_COMMIT` | `--vcs-commit` | global | string | inferred from Git | | | |
  | `APOLLO_VCS_AUTHOR` | `--vcs-author` | global | string | inferred from Git | | | |
  | `APOLLO_ROVER_DEV_ROUTER_VERSION` | `--router-version` | `dev` | version | plugin default | | | |
  | `APOLLO_ROVER_DEV_COMPOSITION_VERSION` | `--composition-version` | `dev` | version | plugin default | | | |
  | `APOLLO_ROVER_DEV_MCP_VERSION` | `--mcp-version` | `dev` | version | plugin default | | | |
  | `APOLLO_ROVER_NO_CONFIG_NOTICES` | `--no-config-notices` | global | boolean | off | | | |

  ● Profile = profile-eligible (§3.6). ● Project = project-eligible (§3.10). ● Net = network-destination setting (§3.9).

- **FR2**: A setting's canonical name is its environment variable's spelling. Rover must use the canonical name in every message it prints about a setting — the inspection verb, the notices of §3.9, and every error. One exception: a message that points at a specific key in a specific file must quote that key exactly as the file spells it, so that a reader can find it by searching for what was printed.
- **FR3**: The following are **not** settings. Each is listed so the audit is complete and so a future reader does not have to rediscover why it was left out.

  | Name | Why it is not a setting |
  |---|---|
  | `APOLLO_KEY`, `APOLLO_CLIENT_ID`, `APOLLO_CLIENT_SECRET` | Credentials. A flag would put the secret in shell history and the process list. |
  | `--profile` | Global flag with no environment variable (FR14). Which profile a command ran against must be visible on the command line, not inherited from the shell. |
  | `--output` | Names this invocation's output file. Not configuration. |
  | `--insecure-accept-invalid-certs`, `--insecure-accept-invalid-hostnames` | An ambient value would make an insecure TLS setting invisible and persistent for every command in a shell. Every insecure invocation stays visible where it is typed. |
  | `APOLLO_ROVER_SKIP_UPDATE`, `--skip-update-check`, `--skip-update` | Already paired, as a set: the variable's meaning is the union of the two flags. No new flag, no new variable, no change to its parsing (FR23). |
  | `APOLLO_ELV2_LICENSE` / `--elv2-license` | Already paired, but per-command rather than process-global, so it falls outside P1.1 rather than being an exception to it. Unchanged by this spec. |
  | `APOLLO_NODE_MODULES_BIN_DIR` | Internal, set by the npm installer. Its only observable effect is which suggestion a "plugin not found" error prints. |
  | `APOLLO_FIRE_FLOWER` | Internal. Absent from the PRD's inventory; recorded here so the catalogue is complete. Not documented, not paired, not stored. |
  | `APOLLO_ROVER` | Set by Rover for a spawned router. Never read by Rover. |
  | `NO_COLOR` | A cross-tool convention Rover mirrors, not a Rover setting. Its parsing is not Rover's to change (FR22). |

- **FR4**: `APOLLO_NODE_MODULES_BIN` is registered as a variable Rover reads and is read nowhere. Rover must stop advertising it: either it acquires a defined effect, or it is removed. It must not remain a name Rover claims to honor and does not.
- **FR5**: `APOLLO_VCS_REMOTE_URL`, `APOLLO_VCS_BRANCH`, `APOLLO_VCS_COMMIT`, and `APOLLO_VCS_AUTHOR` gain flags (FR8) but are neither profile-eligible nor project-eligible. They describe the commit under test, which is a fact about this invocation rather than a property of an environment, and a stored value would silently mislabel every check and publish made from that machine or that clone. This narrows the PRD's A4 candidate list; see §6.
- **FR6**: `APOLLO_GRAPH_REF` is profile-eligible and project-eligible, and a value from either source must drive exactly what the environment variable drives today — enabling GraphOS Router features in a spawned router — and must never be used for schema retrieval. `--graph-ref` keeps its own, unrelated, per-invocation meaning and is not this setting's flag. This resolves the PRD's A4 graph-ref question; see §6.
- **FR7**: `APOLLO_CONFIG_HOME` and `APOLLO_HOME` gain flags but are bootstrap settings: they must never be readable from a profile or a project file, because they determine where that profile and that file are looked for.

### 3.2 Flag / environment variable parity (P1.1–P1.5)

- **FR8**: Every setting in FR1 must be settable from both its flag and its environment variable. A flag marked `global` in FR1 must be accepted by every command; a flag with a narrower scope must be accepted by exactly the commands named and rejected as an unrecognized argument elsewhere.
- **FR9**: For every setting, the flag must win when both the flag and the environment variable are supplied. No setting may apply its own ordering.
- **FR10**: Adding an environment variable to a setting that has a flag with a built-in default must not change what happens when neither is supplied. The default is the default either way.
- **FR11**: A flag's `--help` text must name its environment variable, and the documented environment variable table must name each variable's flag (FR101), so a reader arriving from either side finds the other.
- **FR12**: No existing flag or environment variable may change its meaning, its default, or how its value is parsed. Every addition in this spec is additive.
- **FR13**: A value that is syntactically invalid for its type must be rejected the same way regardless of which of the two supplied it. A flag and its environment variable must not disagree about what counts as a valid value.

### 3.3 The active profile (P1.6, A2)

- **FR14**: `--profile <name>` must be a global flag, accepted by every command. It must have no environment variable equivalent.
- **FR15**: Rover must resolve the active profile exactly once, from the parsed command line, before any command runs, and every part of the invocation — including telemetry, and including commands that have no use for a credential — must use that single resolution.
- **FR16**: Rover must be able to distinguish an explicitly selected profile from the default profile, including when the name selected is literally `default`. Every requirement in §3.5 and §3.9 depends on this distinction.
- **FR17**: A command that has no use for a credential must still accept `--profile` and must still resolve its settings against the selected profile. Accepting the flag and ignoring it entirely is not conforming.
- **FR18**: Resolving settings must not create the configuration directory, a profile directory, or any other file or directory that Rover does not already create. An invocation in a read-only environment with no configuration present must behave exactly as it does today.
- **FR19**: An active profile that has no stored settings, or that does not exist on disk at all, must not be an error. Every setting falls through to the next source in the chain.

### 3.4 Value and boolean conventions (P1.3, A4)

- **FR20**: A boolean environment variable introduced by this spec is set when its value is `1` or `true`, case-insensitive, and unset when it is `0`, `false`, empty, or absent.
- **FR21**: `APOLLO_TELEMETRY_DISABLED` keeps its current presence-only parsing: any value, including `false`, disables telemetry. This is an exception to FR20 and must be documented as one (FR101). See §6.
- **FR22**: `APOLLO_NO_COLOR` keeps its current parsing, which mirrors the cross-tool `NO_COLOR` convention: unset, empty, `0`, and `false` count as unset; anything else counts as set. It is not Rover's convention to normalize.
- **FR23**: `APOLLO_ROVER_SKIP_UPDATE` keeps its current parsing, which already matches FR20.
- **FR24**: A boolean setting stored in a profile or a project file is a typed boolean, regardless of how its environment variable parses. `APOLLO_TELEMETRY_DISABLED` stored as `false` in either store means telemetry is enabled — the opposite of what the environment variable's value `false` means. The inspection verb (§3.8) is what makes this visible, and it must be documented (FR101).

### 3.5 Precedence (A1.3, B1.2)

- **FR25**: For every setting, Rover must take the first source in this chain that supplied a value:

  1. an explicit command-line flag
  2. the environment variable
  3. an explicitly selected profile
  4. the project file
  5. the default profile
  6. Rover's built-in default

  This chain is stated here once and governs every setting in FR1. No command and no setting may apply its own ordering.

- **FR26**: "Explicit flag" means a flag actually passed on the command line. A flag's built-in default is tier 6, not tier 1. For the settings whose flags carry a built-in default today, this distinction is what allows a profile or project value to take effect at all.
- **FR27**: Precedence is decided by presence, not by value. If a higher-precedence source supplied a value, it wins, and whether Rover then accepts or rejects that value follows §3.12.
- **FR28**: An environment variable set to the empty string is present. It wins over every lower source with the empty value, which is today's behavior, and it counts as an override for §3.9.
- **FR29**: Before the project file ships (Part B), tiers 3 and 5 collapse into a single profile tier between the environment variable and the built-in default. The resulting four-tier chain must produce identical results to the six-tier chain for every input in which no project file exists, so that adding the project file later changes nothing for a user who has none.
- **FR30**: This chain governs settings only. Credential resolution is unchanged and is not part of it.
- **FR31**: A setting that is not profile-eligible must never be read from a profile, and a setting that is not project-eligible must never be read from the project file, whatever the store contains. The corresponding tier is simply absent for that setting.
- **FR32**: An explicitly selected profile outranking the project file is not an override for the purposes of §3.9. The user named the profile on the command line; there is nothing to surface.

### 3.6 Profile settings storage (A1.1, A1.2, A2)

- **FR33**: A profile may carry any profile-eligible setting from FR1 alongside its credential.
- **FR34**: Profile settings must be stored in plaintext and must never be placed in the operating system keychain. Reading settings must never trigger a keychain prompt on a command that would not otherwise have touched the keychain.
- **FR35**: Profile settings are stored with the profile. Deleting a profile, and clearing all configuration, must remove its settings along with its credential.
- **FR36**: Re-authenticating a profile and logging out of a profile must preserve its settings. Only the credential changes.
- **FR37**: A profile may exist with settings and no credential. A command that then needs that profile's credential must fail with a message that says so and names how to supply one.

  Required text:
  > Profile `staging` has settings but no credential. Run `rover auth login --profile staging`, or set `APOLLO_KEY` in the environment.

- **FR38**: A profile carrying a setting name this version of Rover does not recognize must produce one warning on stderr and otherwise be ignored, so that a configuration directory shared between two Rover versions does not break the older one.

  Required text:
  > Warning: profile `staging` sets `APOLLO_FUTURE_SETTING`, which this version of Rover doesn't recognize. It will be ignored.

- **FR39**: A profile carrying a *recognized* setting whose stored value fails validation must fail the command (§3.12). It must not be ignored, and it must not be silently replaced by the default.
- **FR40**: A user who has never stored a profile setting must see no behavior change of any kind, including no new files or directories.

### 3.7 `rover config set` and `rover config unset` (A1.6)

- **FR41**: Settings are written through two verbs:
  - `rover config set <SETTING> <VALUE>` — store a value on a profile.
  - `rover config unset <SETTING>` — remove a stored value from a profile.

  Both take `--profile <name>`, defaulting to the default profile, and both accept `--format json`.
- **FR42**: `<SETTING>` is a setting's canonical name, spelled exactly as FR1 spells it. The project file's lowercase alias (FR69) is not accepted here; see §6. A name that is not in FR1, is not profile-eligible, or is the lowercase alias must be rejected before anything is written.

  Required text:
  > `APOLLO_VCS_COMMIT` can't be stored in a profile. It describes a single invocation rather than an environment. Pass `--vcs-commit`, or set `APOLLO_VCS_COMMIT` in the environment.

  > `APOLLO_NOT_A_SETTING` isn't a Rover setting. Run `rover config show` to list the settings Rover recognizes.

  > `apollo_registry_url` isn't a Rover setting name. Settings are named as their environment variables are, so use `APOLLO_REGISTRY_URL`. The lowercase spelling is accepted in `.rover/rover.yaml` only.

- **FR43**: `rover config set` must validate `<VALUE>` syntactically against the setting's type at write time and must refuse to store a value that fails. A malformed URL, a non-numeric timeout, and an unparseable boolean are each rejected when entered rather than when next used.
- **FR44**: Validation is syntactic only. Rover must never contact a host to decide whether a value is acceptable.
- **FR45**: `rover config set` on a profile that does not exist must create it as a settings-only profile with no credential. It must not prompt for a credential and must not fail.
- **FR46**: `rover config unset` on a setting the profile does not carry must be a no-op that says so and exits 0.

  Required text:
  > `APOLLO_REGISTRY_URL` isn't set in profile `staging`. Nothing to remove.

- **FR47**: Neither verb may write a credential, and neither may modify the project file. Existing credential verbs are unchanged, and no new credential verb is added.
- **FR48**: `rover config set` must confirm what it stored, naming the setting, the value, and the profile.

  Required text:
  > Set `APOLLO_REGISTRY_URL` to `https://registry.staging.example.com` in profile `staging`.

- **FR49**: Neither verb emits a §3.9 notice. They act on configuration; they do not use it.

### 3.8 `rover config show` (A1.4)

- **FR50**: `rover config show` must report, for every setting in FR1, its effective value and the source that supplied it, for the profile the invocation resolved to. It takes `--profile <name>` and `--format json`.
- **FR51**: `source` must be exactly one of six literals, one per tier of FR25: `flag`, `environment`, `explicit_profile`, `project_file`, `default_profile`, `builtin`.
- **FR52**: JSON output must additionally report, per setting, every lower-precedence source that also supplied a value and what it supplied. A single "winner" field cannot show an override, and the override is the thing a user running this verb is most often trying to see.
- **FR53**: The JSON payload must travel in Rover's standard envelope:

  ```json
  {
    "json_version": "1",
    "data": {
      "profile": "staging",
      "profile_selection": "explicit",
      "credential": { "present": true, "origin": "profile" },
      "settings": [
        {
          "name": "APOLLO_REGISTRY_URL",
          "value": "https://registry.staging.example.com",
          "source": "explicit_profile",
          "overridden": [
            { "source": "project_file", "value": "https://mirror.example.com" }
          ]
        },
        {
          "name": "APOLLO_CHECKS_TIMEOUT_SECONDS",
          "value": "300",
          "source": "builtin",
          "overridden": []
        }
      ],
      "success": true
    },
    "error": null
  }
  ```

  `profile_selection` is `explicit` or `default`. `overridden` is ordered by precedence, highest first, and is `[]` when nothing was overridden.
- **FR54**: Every `value` in that payload is a JSON string, spelled the way the setting's environment variable would spell it, whatever the setting's type. A consumer gets one type for every setting rather than a per-setting type it would have to know in advance, and the spelling round-trips: the reported string is a value that could have been exported to produce it.
- **FR55**: `rover config show` must never print a credential value, masked or otherwise. It reports only whether a credential is present and where it came from.
- **FR56**: `rover config show` must emit no §3.9 notice, for any setting, under any circumstances. It reports overrides as data; a notice about the report would be noise.
- **FR57**: With no configuration present anywhere, `rover config show` must report every setting with source `builtin`, and must create nothing (FR18).
- **FR58**: Text output must carry the same value and source for every setting as the JSON does. It is not required to render the `overridden` detail, which is the machine-readable half of the contract.

### 3.9 Configuration notices (A1.5, B1.4)

- **FR59**: Rover must print a one-line notice to stderr when, for a setting the current invocation **uses**, either of these holds:
  - **(a)** an environment variable overrode a value that an *explicitly selected* profile also supplied; or
  - **(b)** a profile or the project file supplied a non-default value for a network-destination setting.
- **FR60**: "Uses" is not "resolves." A notice fires only when the invocation acts on the value — for a network-destination setting, when it issues at least one request to that destination during the run. A command that resolves a setting and never acts on it emits nothing.
- **FR61**: At most one notice per setting per process, printed on first use. When both (a) and (b) apply to the same setting, one notice states both.
- **FR62**: Notices must never appear inside the JSON envelope, must not be suppressed by `--format json`, and must never affect the exit code.
- **FR63**: Notices may be suppressed only by `--no-config-notices` or `APOLLO_ROVER_NO_CONFIG_NOTICES`. No value stored in a profile or a project file may suppress them — a "quiet this" key in the same file an attacker would tamper with defeats the purpose of surfacing.
- **FR64**: Required text, case (b), profile source:

  > Note: profile `staging` sets `APOLLO_REGISTRY_URL` to `https://registry.staging.example.com`.

- **FR65**: Required text, case (b), project-file source:

  > Note: `.rover/rover.yaml` sets `APOLLO_ROVER_DOWNLOAD_HOST` to `https://mirror.example.com`.

- **FR66**: Required text, case (a):

  > Note: `APOLLO_REGISTRY_URL` from the environment overrides the value set in profile `staging`.

- **FR67**: Required text, both cases for one setting:

  > Note: `APOLLO_REGISTRY_URL` from the environment is set to `https://registry.staging.example.com`, overriding the value set in profile `prod`.

### 3.10 The project file (B1.1, B1.3)

- **FR68**: Rover's project manifest may carry a top-level `settings:` section whose keys are setting names from FR1 and whose values are that setting's type:

  ```yaml
  settings:
    APOLLO_REGISTRY_URL: https://registry.example.com
    APOLLO_ROVER_DOWNLOAD_HOST: https://mirror.example.com
    APOLLO_CHECKS_TIMEOUT_SECONDS: 600
  ```

- **FR69**: A key under `settings:` may be spelled either as the setting's canonical name or as that name's exact all-lowercase form. `APOLLO_REGISTRY_URL` and `apollo_registry_url` are the same key, and both must be accepted:

  ```yaml
  settings:
    APOLLO_REGISTRY_URL: https://registry.example.com
    apollo_rover_download_host: https://mirror.example.com
  ```

  No other spelling is accepted. Mixed case, kebab-case, and a name with the `APOLLO_` prefix dropped are each an unrecognized key (FR72). This is not a general case-insensitivity rule, and it applies to the project file alone.
- **FR70**: A `settings:` section that spells the same setting both ways must fail the command, naming the setting and both spellings. Rover must not pick one and must not merge them.

  Required text:
  > `.rover/rover.yaml` sets `APOLLO_REGISTRY_URL` twice, once as `APOLLO_REGISTRY_URL` and once as `apollo_registry_url`. These are the same setting. Remove one.

- **FR71**: The `settings:` section is hand-authored. No Rover command writes it, and none is added that does.
- **FR72**: A key inside `settings:` that is not a setting name from FR1, or that is not project-eligible, must produce one warning on stderr and otherwise be ignored — the same rule as FR38. This is distinct from an unrecognized *top-level* manifest key, which the plugin system's rule ignores without warning.
- **FR73**: A credential name under `settings:` must be an error, not a warning, and must fail the command (§3.12).
- **FR74**: A recognized key under `settings:` whose value fails validation must fail the command naming the file and the key (§3.12).
- **FR75**: A network-destination value from the project file applies as-is: no confirmation, no allowlist, no per-user trust record, identically in an interactive terminal and in CI. FR65's notice is what makes the redirect visible, and it must fire on every invocation that sends a request there.
- **FR76**: A `settings:` section in the user-level manifest must be ignored with one warning. Profiles are the user-level settings store; honoring a second one would reintroduce the sprawl this initiative exists to remove.

  Required text:
  > Warning: the user-level `rover.yaml` has a `settings:` section, which Rover ignores. Use `rover config set` to store user-level settings in a profile.

- **FR77**: The project file must never be able to select a profile. There is no setting, and no other key under `settings:`, that changes which profile an invocation runs against.

### 3.11 Sharing the manifest with the plugin system (B2)

- **FR78**: The project file is found by the plugin system's discovery rule and by no other. This spec defines no discovery of its own, no second search path, and no per-command flag that relocates only the settings half of the file. A command that can redirect the manifest redirects both sections together or neither.
- **FR79**: Manifests are never merged. The nearest manifest that discovery finds is the project file, whether or not it contains a `settings:` section.
- **FR80**: Each section's rules cover only that section. This spec's validation, eligibility, and notice rules apply to `settings:` and to nothing else in the manifest; the plugin section's rules apply to it and not to `settings:`. The two precedence chains — plugin version resolution and settings resolution — stay independent.
- **FR81**: Any Rover command that writes the manifest must preserve the other owner's section exactly as its author left it, including comments and formatting.
- **FR82**: `APOLLO_ROVER_DOWNLOAD_HOST` under `settings:` redirects where plugin binaries are fetched from. This interaction is intended and permitted. FR65's notice surfaces the redirect on every invocation that downloads from it, and the plugin system's checksum verification is the integrity control on what arrives.

### 3.12 Validation and the error contract (A2, B2)

- **FR83**: Rover must validate a stored setting's value syntactically when it is read, and must fail the command when a recognized setting's stored value is invalid. It must never fall back to the default, and never to the next source in the chain.
- **FR84**: A validation failure must name the source and the setting, and must name a concrete way to correct it.

  Required text, profile source:
  > `APOLLO_CHECKS_TIMEOUT_SECONDS` in profile `staging` is set to `soon`, which isn't a whole number of seconds. Run `rover config set APOLLO_CHECKS_TIMEOUT_SECONDS <seconds> --profile staging` to correct it.

  Required text, project-file source:
  > `.rover/rover.yaml` sets `APOLLO_REGISTRY_URL` to `registry.example.com`, which isn't a valid URL. URLs must include a scheme, for example `https://registry.example.com`.

- **FR85**: A credential name under `settings:` must fail with its own message.

  Required text:
  > `.rover/rover.yaml` sets `APOLLO_KEY` under `settings:`. Credentials can't be stored in a project file. Run `rover auth login`, or set `APOLLO_KEY` in the environment.

- **FR86**: Three failure classes must have their own stable error codes, surfaced as `error.code` in JSON output and in the printed error:
  1. A stored setting's value failed validation (FR84).
  2. A credential was named in the project file (FR85).
  3. The project file spelled one setting both ways (FR70).
- **FR87**: All three codes must be documented in the generated error reference, with the same page-per-code treatment every other Rover error code gets.
- **FR88**: Rover must never check a setting's value for reachability. A URL that parses is accepted; whether anything answers at it is the invocation's problem, reported by whatever fails when Rover tries.
- **FR89**: An unrecognized setting name warns and is ignored, at every level (FR38, FR72). An unrecognized name is never an error, because that is what allows one configuration directory or one repository to be shared across Rover versions.

### 3.13 Child-process forwarding (A2, B2)

- **FR90**: Where Rover forwards a setting's environment variable to a process it spawns today, a value for that setting resolved from a profile or the project file must be forwarded identically, as if it had been set in the environment.
- **FR91**: A setting Rover does not forward today must not become forwarded because it was resolved from a profile or the project file.

### 3.14 Compatibility (P1.5, A1.2, B5)

- **FR92**: No existing flag or environment variable changes its meaning, its default, or how its value is parsed.
- **FR93**: A user with no profile settings and no project file must observe behavior identical to today's in every respect, and Rover must create no file or directory it does not create today.
- **FR94**: A CI job supplying only a credential through the environment, with no configuration directory at all, must work exactly as it does today.
- **FR95**: Rover's existing end-to-end suite must pass unchanged when run with the configuration directory pointed at an empty location and no project file in any ancestor of the working directory.
- **FR96**: No requirement in this spec is a breaking change.

### 3.15 Documentation (P1.4, A5, A1.3)

- **FR97**: The published supported-environment-variables table must list every setting in FR1 with its flag, including every variable that is undocumented today. Where the project file is documented, the lowercase alias (FR69) must be stated, along with the fact that it is accepted there and nowhere else.
- **FR98**: The current blanket precedence sentence — "If present, an environment variable's value takes precedence over all other methods of configuring the associated behavior" — must be replaced by FR25's chain. It is inaccurate today, independently of anything this spec adds, so the correction ships with the first slice rather than waiting for the last.
- **FR99**: The credential resolution chain must be documented separately from FR25's chain, and stated to be unaffected by it.
- **FR100**: Each flag's `--help` text must name its environment variable, and the docs must name each variable's flag (FR11).
- **FR101**: The boolean exceptions must be documented where a reader will hit them: `APOLLO_TELEMETRY_DISABLED`'s presence-only parsing (FR21), `APOLLO_NO_COLOR`'s mirroring of `NO_COLOR` (FR22), and the fact that a stored boolean is a typed boolean and so reads `false` differently from the environment variable of the same name (FR24).
- **FR102**: No configuration behavior may be documented in `--help` or in published docs that the shipping code does not implement.

---

## 4. Acceptance criteria

**Flag beats environment beats profile beats default**
Given profile `staging` sets `APOLLO_REGISTRY_URL` to `https://profile.example.com`, when `rover graph check --profile staging` runs, then the profile's URL is used. Given additionally `APOLLO_REGISTRY_URL=https://env.example.com`, then the environment's URL is used. Given additionally `--registry-url https://flag.example.com`, then the flag's URL is used. With none of the three set, the built-in default is used.

**An explicit profile beats the project file, which beats the default profile**
Given a project file setting `APOLLO_REGISTRY_URL` to `https://repo.example.com`, and profile `staging` setting it to `https://staging.example.com`, and profile `default` setting it to `https://default.example.com`: `rover graph check --profile staging` uses `https://staging.example.com`; `rover graph check` uses `https://repo.example.com`; and with the project file removed, `rover graph check` uses `https://default.example.com`.

**`--profile default` typed literally is explicit**
Given the same setup, when `rover graph check --profile default` runs, then `https://default.example.com` is used, not the project file's value, and `rover config show --format json` reports `"profile_selection": "explicit"`.

**An empty environment variable is present**
Given profile `staging` sets `APOLLO_REGISTRY_URL` and `APOLLO_REGISTRY_URL=` is exported, when a command runs with `--profile staging`, then the empty value wins, `rover config show` reports source `environment` with an empty value, and the case-(a) notice fires.

**A setting that is not profile-eligible is never read from a profile**
Given a profile store that somehow contains `APOLLO_VCS_COMMIT`, when any command runs against that profile, then the value is ignored, `rover config show` reports that setting's source as one of the tiers that did supply it, and the FR38 unknown-setting warning is printed.

**An unconfigured user is unaffected**
Given no profile settings, no project file, and no new flags or variables, when any command runs, then its behavior is identical to today's and no file or directory is created. Given a read-only configuration directory, the same holds and the command does not fail.

**A settings-only profile**
Given no profile named `staging`, when `rover config set APOLLO_REGISTRY_URL https://staging.example.com --profile staging` runs, then the profile is created with that setting and no credential, and the command exits 0 without prompting. When `rover graph check --profile staging` then runs with no credential in the environment, it fails with the FR37 text.

**A missing profile falls through**
Given `--profile nonexistent` and no such profile on disk, when a command runs with only `APOLLO_KEY` set, then it succeeds, every setting resolves from the next available source, and no error or warning about the missing profile is printed.

**Write-time validation**
Given any profile, when `rover config set APOLLO_REGISTRY_URL registry.example.com` runs, then nothing is stored and the invalid-URL message is printed. When `rover config set APOLLO_CHECKS_TIMEOUT_SECONDS soon` runs, then nothing is stored and the non-numeric message is printed. When `rover config set APOLLO_VCS_COMMIT abc123` runs, then it is rejected with the FR42 text.

**Read-time validation fails the command**
Given a profile whose stored `APOLLO_CHECKS_TIMEOUT_SECONDS` was made invalid by hand, when a command that uses it runs, then the command fails with the FR84 text and the invalid-value error code, which appears under `error.code` in `--format json`. It must not fall back to `300`.

**An unknown setting warns and is ignored**
Given a profile carrying `APOLLO_FUTURE_SETTING`, when any command runs against it, then the FR38 warning is printed once, the command succeeds, and the exit code is 0. The same holds for an unknown key under `settings:` in the project file.

**`rover config show` reports every source**
Given one setting from each of the six tiers, when `rover config show --format json` runs, then each reports the matching `source` literal, and the setting supplied by two sources reports the loser under `overridden`. No credential value appears anywhere in the output.

**`rover config show` on a clean machine**
Given no configuration of any kind, when `rover config show` runs, then every setting in FR1 is reported with source `builtin`, nothing is created on disk, and the exit code is 0.

**A notice fires once, and only on use**
Given profile `staging` sets `APOLLO_ROVER_DOWNLOAD_HOST` to a non-default host, when a command that downloads a plugin runs with `--profile staging`, then exactly one FR64 notice is printed to stderr regardless of how many artifacts are fetched, and the exit code is unchanged. When a command that downloads nothing runs, then no notice is printed.

**A notice is not suppressed by `--format json`**
Given the same setup, when the command runs with `--format json`, then the notice appears on stderr, nothing about it appears in the JSON envelope, and the JSON parses.

**Suppression comes only from a non-persisted source**
Given the same setup, when `APOLLO_ROVER_NO_CONFIG_NOTICES=true` is set or `--no-config-notices` is passed, then no notice is printed and nothing else about the invocation changes. Given instead a profile or project-file key attempting to suppress notices, then the notice is still printed and the unknown-setting warning is emitted for that key.

**An explicit profile outranking the project file is silent**
Given a project file setting `APOLLO_REGISTRY_URL` and profile `staging` setting it too, when `rover graph check --profile staging` runs against the registry, then the only notice printed is the FR64 notice for the profile's own value. Nothing mentions the project-file value it outranked.

**The environment overriding an explicit profile is surfaced**
Given `APOLLO_REGISTRY_URL=https://env.example.com` and profile `prod` setting it to something else, when `rover graph check --profile prod` runs, then the FR67 notice is printed once on stderr and the run proceeds against the environment's registry with `prod`'s credential.

**The project file accepts either spelling, but not both**
Given a project file whose `settings:` section uses `apollo_registry_url`, when a command runs in that project, then the value applies exactly as the canonical spelling would, and `rover config show` reports the setting as `APOLLO_REGISTRY_URL`. Given `Apollo_Registry_Url` instead, then the key is unrecognized, the FR72 warning is printed, and the value is not applied. Given a file carrying both `APOLLO_REGISTRY_URL` and `apollo_registry_url`, then the command fails with the FR70 text and neither value is applied.

**The lowercase alias stops at the file**
Given any profile, when `rover config set apollo_registry_url https://example.com` runs, then nothing is stored, and the message names `APOLLO_REGISTRY_URL` as the spelling to use and says where the lowercase form is accepted.

**A credential in the project file fails**
Given a project file with `APOLLO_KEY` under `settings:`, when any command runs in that project, then it fails with the FR85 text and the credential-in-project-file error code, and no request is made.

**The user-level manifest's `settings:` is ignored**
Given a `settings:` section in the user-level manifest, when any command runs, then the FR76 warning is printed once, none of those values is applied, and the command succeeds.

**Both manifest sections survive a write**
Given a project manifest with both a plugin section and a `settings:` section, when a Rover command that writes the manifest runs, then the `settings:` section is byte-for-byte unchanged, including its comments and key order.

**A project file makes two machines agree**
Given a repository containing a project file and no other configuration, when two users on two different machines run `rover config show --format json` from a fresh clone, then every setting's value and source pair is identical between them.

**Forwarding follows the environment variable**
Given profile `staging` sets `APOLLO_GRAPH_REF`, when `rover dev --profile staging` spawns a router, then the router receives that graph ref in its environment exactly as it would have had the variable been exported, and Rover itself does not use it to retrieve any schema.

**The four-tier chain matches the six-tier chain**
Given any configuration containing no project file, when a Rover build with the Part A chain and a Rover build with the full chain resolve the same invocation, then every setting's value and source agree.

---

## 5. Non-goals

- Deprecating or removing any existing `APOLLO_*` environment variable or flag.
- Changing credential resolution: the order in which the API key, OAuth client credentials, and the stored profile credential are tried is untouched.
- Moving `APOLLO_CONFIG_HOME` or `APOLLO_HOME` into profile or project storage (FR7).
- Defining the on-disk format of profile settings storage.
- Defining the project manifest's path, its discovery rule, or its plugin sections. The plugin system owns them; this spec adds a `settings:` section and nothing else.
- Honoring `settings:` in the user-level manifest (FR76).
- A global, non-profile user settings file for machine-level preferences. A setting that fails the eligibility test is out of scope, not relocated.
- Storing credentials of any kind in the project file, or adding a third persisted credential kind to the profile.
- A project file selecting a profile (FR77).
- Merging multiple project manifests (FR79).
- A trust prompt or allowlist for network destinations supplied by a project file (FR75).

---

## 6. Decisions

Decisions taken while drafting, and their reasoning.

- **`show` / `set` / `unset`**, over `get` / `set` / `unset`. `get` reads as a single-value lookup, which would make listing the whole configuration — the common case, and the one the PRD's legibility goal is about — the odd one out. `rover config list` already means "list profiles," so `show` is unambiguous. Nesting under a `settings` noun was considered and rejected as a third word on every invocation for no added clarity.

- **The canonical name is the environment variable's spelling; the project file also accepts its lowercase form** (FR2, FR69), over a single uppercase-only spelling and over giving the file a vocabulary of its own.

  Two spellings with a mechanical mapping is the norm, not a hazard: Cargo has `CARGO_BUILD_JOBS` alongside `build.jobs`, npm has `npm_config_<key>` alongside lowercase `.npmrc` keys. But those tools earn the second spelling — their config files are hierarchical, and the environment variable is a flattening of a path. Rover's settings are flat, so a file-specific vocabulary would be `tolower()` of the canonical name and nothing else.

  Making lowercase the *only* file spelling was rejected for a specific reason. Once the name is lowercased, the `APOLLO_` prefix is dead weight — `settings:` already namespaces it — and the next question is why it is still there. Answering that means choosing a short name per setting, and those are not mechanical: `APOLLO_ROVER_DOWNLOAD_HOST` shortens just as reasonably to `download_host` as to `rover_download_host`. That is the per-setting translation table this initiative exists to remove, arrived at one reasonable step at a time.

  Accepting both spellings in the file, and only in the file, takes the aesthetic objection seriously where it actually applies — a hand-edited YAML file whose other keys are lowercase — while keeping one name in the contract and one name in everything Rover prints. The verbs of §3.7 are excluded deliberately: a command-line argument that names an environment variable conventionally spells it as one, and no competing convention pulls the other way there. The cost is worth naming rather than hiding: a reader can meet a project file that does not match the documented spelling, and "we accept both" is a small smell. FR70 is what keeps it from becoming a real one, by refusing a file that uses both at once instead of silently picking a winner.

- **`APOLLO_VCS_*` gets flags but is neither profile- nor project-eligible** (FR5), narrowing the PRD's A4 candidate list. The eligibility test is whether switching org or environment would plausibly require changing the setting, and the commit under test does not vary by org — it varies by invocation. A stored value would be wrong for every invocation after the one it was written for, and would mislabel checks and publishes silently rather than loudly. Pairing them with flags is still worth doing on its own: it is the Prerequisite's whole point, and a flag is per-invocation by construction.

- **`APOLLO_GRAPH_REF` drives router-feature enablement only, never schema retrieval** (FR6), resolving the PRD's A4 graph-ref question. Since a setting is named for its environment variable, it must behave as that variable behaves, or the naming convention becomes a trap. The variable enables router features; `--graph-ref` selects what a command acts on. Letting a stored value quietly choose which graph a publish targets would turn a configuration file into a targeting mechanism, which is the one thing the PRD is most careful to keep on the command line.

- **`APOLLO_TELEMETRY_DISABLED` keeps presence-only parsing** (FR21), resolving the PRD's A4 normalization question, over normalizing it to the shared convention. Normalizing would mean `APOLLO_TELEMETRY_DISABLED=false` starts *enabling* telemetry for people who set it precisely to disable telemetry. That is a breaking change (FR12 forbids it), it fails in the privacy-losing direction, and it fails silently. The exception is documented instead (FR101). The new `--telemetry-disabled` flag is a bare boolean, so the flag side never has to parse a value at all.

- **A stored boolean is a typed boolean** (FR24), accepting that `APOLLO_TELEMETRY_DISABLED: false` in a profile means the opposite of `APOLLO_TELEMETRY_DISABLED=false` in the environment. The alternative — propagating presence-only semantics into a YAML file, where a key's presence and its value are separately meaningful — is worse, because there is no way to write "explicitly enabled" at all. The inspection verb shows the effective value, which is the honest fix.

- **`APOLLO_ROVER_DEV_*_VERSION` stays out of both stores** (FR1). The plugin system's manifest is the declared home for plugin version declarations, with its own precedence chain and its own lockfile. A second store for the same fact, with a different chain, would recreate the sprawl problem one layer down and produce two sources of truth that disagree.

- **`APOLLO_CLIENT_TIMEOUT` is eligible; `APOLLO_LOG_LEVEL` and `APOLLO_FORMAT` are not** (FR1). Request timeouts plausibly differ between a fast production environment and a slow staging one, which is the eligibility test passing. Log verbosity and output format are properties of the person and the terminal, not of the org — they belong on the command line or in a shell, and storing them per profile would surprise anyone who switched profiles and found their output format had changed.

- **OAuth client credentials are out of scope**, resolving the PRD's A4 question by declining it. They are a credential, not a setting, and making them a third persisted credential kind is a credential-model decision with its own threat model. The PRD already notes it may warrant its own PRD; this spec records that as the answer rather than leaving the question open.

- **Notices are suppressed by `--no-config-notices` / `APOLLO_ROVER_NO_CONFIG_NOTICES`** (FR63), both non-persisted. A suppression key inside the file an attacker would tamper with defeats the surfacing entirely. Organizations running a mirrored registry already configure it in shell profiles and CI definitions; the suppression goes in the same place, where it is visible in the same review.

- **`rover config show` reports what was overridden, in JSON** (FR52), over reporting only the winning source. The override is the PRD's acknowledged footgun, and a single source field can state the winner but not the fact that something lost. Text output is exempt because a per-setting stack does not fit a table a person reads at a glance.

- **`rover config show` emits no notices** (FR56), over having it emit them for the settings it reports. The verb's entire job is reporting configuration; a notice would duplicate its own output, and it would fire for settings this invocation is not using, which contradicts FR60.

- **Symlinked directories and git worktrees are ordinary directories** (FR78), resolving the PRD's B4 discovery question. No canonicalization, no special case: whatever the plugin system's upward walk finds from the working directory as given is the project. Adding settings-specific path handling would create exactly the two-rules-for-one-file situation FR78 exists to prevent, and a worktree that wants its own configuration can contain its own `.rover/`.

- **Whichever initiative arrives first implements the one discovery rule** (FR78), resolving the PRD's B4 sequencing question. None of the plugin system's manifest, lockfile, or project-level discovery is built yet, so "Part B waits for ROVER-420" and "Part B lands the shared discovery" are both live options and the choice is a scheduling one. What is not negotiable is the outcome: one rule, implemented once, consumed by both sections. This spec therefore states the constraint and declines to state the order.

- **Slicing is constrained by one requirement, not by a fixed setting order** (FR29), resolving the PRD's A4 delivery-slicing question. Which settings a given slice enables is a scheduling choice and this spec deliberately does not fix it; what a slice may not do is change a resolved value for a user who has no project file. FR29 makes that checkable — the four-tier chain and the six-tier chain must agree on every such input — so slices can be reordered freely without anyone having to re-derive whether the reordering is safe.

- **The catalogue is the complete list, and corrects the PRD's inventory** (FR1, FR3). Drafting turned up `APOLLO_FIRE_FLOWER`, which the PRD's survey missed; `APOLLO_NODE_MODULES_BIN`, which Rover advertises and never reads (FR4); and `APOLLO_NODE_MODULES_BIN_DIR`, whose only observable effect is which suggestion an error prints, not install paths as the PRD describes. Recording these is the point of writing the catalogue as a single normative table rather than prose: it is the artifact that makes "every setting is documented" checkable.

---

## 7. Test obligations

This contract cannot be verified by unit tests alone. Four gaps must be closed alongside the work:

- **Independent control of both stores.** Precedence across six tiers cannot be exercised without fixtures that point the configuration directory at a temporary location *and* place or withhold a project file in an ancestor of the working directory, independently, in one test. Neither half exists today.
- **A no-writes assertion.** FR18 and FR93 say Rover creates nothing when reading settings. That is not observable in a test that merely checks a command succeeded; it needs a fixture that fails if the configuration directory gains an entry, and one that runs against a read-only directory.
- **Snapshot coverage of the inspection envelope.** FR51's six `source` literals and FR53's shape are a machine-readable contract that scripts will match on, so they need snapshot coverage that fails on a rename rather than field-by-field assertions that a regression can slip past.
- **Cross-platform coverage of project-file discovery.** Path walking is where Unix and Windows diverge, and the acceptance criteria that depend on finding — or deliberately not finding — a project file in an ancestor directory need to run on every supported platform, not only on the developer's.
