# PRD: Rover Telemetry

- **Owner:** Brian George
- **Component:** Rover CLI (anonymous usage telemetry)

This PRD reviews Rover's current usage telemetry, benchmarks it against other industry CLI applications, and recommends where Rover should land. It fixes disclosure and data-minimization gaps and adds the fields needed for aggregate analysis. Request tracing against the Platform API is a separate concern and is covered by its own PRD.

One decision shapes this PRD: **telemetry stays anonymous.** No other industry CLI we surveyed ties usage telemetry to authenticated account identity. If we want user-to-platform usage correlation, server-side capabilities should achieve it without Rover sending identity.

## 1. Problem statement

Rover's telemetry is a single fire-and-report request per invocation. Measured against Rover's published "Privacy and Data Collection" documentation (`docs/source/privacy.mdx`) and against other industry CLI applications, it has four concrete problems:

1. **The published privacy documentation is inaccurate.** It claims identifiable arguments are excluded from what Rover collects, but that claim does not fully hold: some argument values are sent unscrubbed.
2. **Telemetry can delay every command.** Reporting runs in the background, but the process always waits for it before exiting. On a slow or unreachable network, every command can be delayed by up to the full request timeout, contradicting the intent that telemetry is not in the critical path.
3. **The payload is too thin to be useful for product decisions.** There is no command duration, no success-or-error outcome, no way to group invocations by graph, and only a coarse CI-vendor signal for telling end-user invocations from automated ones. Earlier internal proposals asked for duration, outcome, and derived feature-usage fields; none were built.
4. **No enterprise or regulated-customer exemption.** Telemetry is sent regardless of where the user is authenticated. At least one widely used industry CLI skips telemetry entirely when authenticated against a self-hosted enterprise server.

## 2. Background: how this works today

- **One report per invocation.** When enabled, Rover sends one request to Apollo's telemetry endpoint per invocation, with a 1-second timeout, no batching, and **no sampling**. Debug builds never report.
- **Opt-out only.** Telemetry can be disabled by the `APOLLO_TELEMETRY_DISABLED` environment variable, which is presence-based (any value, including `false`, disables it); by the `--telemetry-disabled` flag; by a setting stored on a profile; or by a setting in the project file. The stored forms are real booleans, so `false` there keeps telemetry enabled. The privacy documentation lists all four.
- **What is sent.** Each event contains:
  - The command name and its arguments, serialized without scrubbing except for fields already excluded from serialization (for example graph ref, profile name, and subgraph name, URL, and schema path)
  - A persistent anonymous machine identifier
  - A per-invocation session identifier
  - A SHA-256 hash of the working directory
  - A SHA-256 hash of the Git remote URL, when one exists
  - The operating system and CPU architecture
  - The CI vendor, when detected
  - The Rover version
- **What is not sent.** Duration, outcome (success or error), derived feature usage, or any account, org, or graph identifier.
- **Telemetry is disconnected from authentication.** No identity plumbing exists between telemetry and Rover's auth subsystem, even experimentally.
- **Error codes are not classified.** Rover's error codes are a flat list with no notion of user-caused versus internal or platform-caused errors. Rust panics bypass the error-code path entirely, so today a panic produces no outcome at all.

### Industry practice

A survey of other industry CLI applications (a source-hosting CLI, a cloud-provider CLI, and a second cloud-provider CLI) shows:

- **Defaults vary.** One is on by default with opt-out, and was widely criticized for shipping with no first-run disclosure. One asks for opt-in at install and persists the answer. One has no telemetry in its core CLI at all.
- **Identifiers are pseudonymous and never account-linked.** Where an identifier exists, it is a device and per-invocation identifier, or an undocumented "anonymized" one.
- **Values are not collected.** Where fields are documented, they are command, flag names (not values), OS and architecture, CI detection and version, and in one case duration and an error-occurred flag. One explicitly excludes argument values and PII. One samples about 1% of invocations.
- **Opt-out is easy and multi-path.** Environment variable, the cross-tool `DO_NOT_TRACK` convention, and a persisted config setting are all offered, depending on the tool.
- **Enterprise handling is rare.** One skips telemetry entirely when authenticated against a self-hosted enterprise server; the others don't document any.

## 3. Shared definitions

- **Telemetry event.** The single anonymous report Rover sends for one invocation.
- **Identifying value.** A flag or argument value that could identify a person, organization, machine, or project: file-system paths, profile names, URLs, graph refs, keys, and free-text values.
- **Allowlisted flag value.** The value of a flag on an explicit allowlist of flags whose values are provably non-identifying (for example `--format`).
- **Resource-scoped identifier.** A hash of something a user works on (a graph, a working directory, a repository), as opposed to an identity-scoped identifier, which would name a person or account.
- **Outcome.** One of success, user error, or internal error.
- **Invocation environment.** A coarse classification of the context an invocation ran in, at minimum interactive end-user versus automated.

## 4. Goals

1. **Accurate disclosure.** The "Privacy and Data Collection" documentation (`docs/source/privacy.mdx`) makes no claim about exclusions that the payload does not honor, and documents every field Rover sends, including any added by this PRD. A reader can compare the docs to the payload and find no discrepancy.
2. **No unscrubbed argument values.** Rover reports flag names only, plus the values of allowlisted flags. No identifying value is sent, including values not already excluded from serialization today. This makes the privacy documentation's existing claim true.
3. **Bounded exit delay.** Telemetry never delays process exit by more than a small, fixed cap on a slow or unreachable network, and still delivers reliably in the normal case where the request completes in tens of milliseconds. Telemetry must not silently drop events for commands that finish in a few milliseconds.
4. **Duration.** Each event includes the wall-clock duration of the command's execution.
5. **Graph identifier.** When a graph ref is resolvable for the invocation, the event includes a one-way identifier derived from it, so invocations can be grouped by graph without revealing the graph. When no graph ref is resolvable, the field is absent. The identifier is disclosed in the privacy documentation alongside the other fields. How it is derived so that it is genuinely anonymous is an open question (section 7).
6. **Outcome classification.** Each event includes the command's outcome: success, user error, or internal error. Panics are in scope: a command that panics reports an internal error.
7. **Invocation environment.** Each event includes the invocation environment, so end-user and automated use cases can be analyzed separately. The classification is coarse, anonymous, and more reliable than today's CI-vendor detection alone. It never includes hostnames, usernames, or paths.
8. **Full backward compatibility for opt-out.** `APOLLO_TELEMETRY_DISABLED` continues to disable telemetry exactly as it does today. Everything added is additive.

## 5. Key decisions and rationale

- **Shrink the exit delay; don't remove it.** Fully detaching the reporting task risks the process exiting before a fast command's request has even connected. For commands that finish in milliseconds, which is likely faster than name resolution plus a TLS handshake, that would silently drop events, which is worse than the current delay. Keeping the wait but lowering the request timeout well below one second (candidate range 150 to 300 milliseconds) caps the worst-case cost on a bad network at something barely noticeable while keeping delivery reliable in the normal case.
- **Flag names by default, values by allowlist.** This matches the strictest posture among the CLIs surveyed and is the only one that can be shown correct without auditing every flag's value space. A flag whose values are provably non-identifying can opt in by name. Everything else, including any new flag, is name-only by default, so a new flag can never leak a value by omission.
- **The graph identifier is resource-scoped, not identity-scoped.** It is the one new field needed to aggregate usage by graph cohort, and cohort aggregation is the substitute for account-linked identity (see the Non-goals). A plain unsalted hash would not be anonymous, because anyone able to enumerate known graph refs could hash them and match, so the derivation must resist that. It is therefore an open question rather than a settled design.
- **Duration and outcome have direct industry precedent.** At least one surveyed CLI sends both; neither requires identity.
- **User error versus internal error is new classification work.** No existing error code is marked as user-caused or internal. The mapping has to be decided for every error. Panics need a separate path because they never become an error code, so the spec must say how a panic is captured and reported before the process exits.
- **Environment is a classification, not a fingerprint.** Rover can already infer CI from the CI vendor, flags, and configuration, but that is incomplete. A coarse interactive-versus-automated value answers the product question without collecting anything that identifies a machine or person.
- **Anonymity is preserved.** No field ties an event to an account, org, or person.

## 6. Non-goals

- A persistent cross-session user identifier (`user_id`). Dropped this round; reconsider only if cohort aggregation is tried and found genuinely insufficient.
- Account- or org-level fields.
- Capturing responses to interactive prompts.
- Changing the default (on, opt-out) or adding an opt-in prompt.
- Sampling. Whether to sample is an open question (section 7) rather than a goal.
- Enterprise or regulated-customer exemption. It is an open question (section 7) rather than a goal.
- Request tracing against the Platform API. It is covered by its own PRD.

## 7. Open questions (resolve in the spec)

- **Timeout value.** The exact cap on the telemetry request, within the 150 to 300 millisecond candidate range.
- **Allowlist contents.** Which flags have values provably non-identifying enough to report.
- **Error classification.** Which error codes are user errors and which are internal, and what happens to errors that are plausibly both.
- **Panic capture.** How a panic is observed and reported before the process exits, given it never becomes an error code.
- **Graph identifier derivation.** How to make the identifier anonymous rather than merely hashed: whether it is salted or keyed on the client, what the salt or key scope is, and what grouping that preserves across users.
- **Invocation environment signals.** Which signals feed the classification (for example whether a terminal is attached, CI vendor, container markers), and how coarse the reported value is.
- **Enterprise exemption.** Whether Rover should skip telemetry for non-default registries or enterprise environments, and how it would know.
- **Sampling.** Whether reporting 100% of invocations is still justified once duration and outcome make events more useful.

## 8. Success metrics

- **Disclosure.** Every field in the payload appears in the privacy documentation, and every field described there is in the payload. The graph identifier and invocation environment are listed, and no exclusion claim in the documentation is contradicted by the payload.
- **Minimization.** For any invocation, no flag value outside the allowlist appears in the event, verified against the full set of global and per-command flags.
- **Exit delay.** On an unreachable telemetry endpoint, no command's exit is delayed by more than the chosen cap.
- **Delivery.** On a reachable endpoint, events from commands that finish in a few milliseconds are delivered at the same rate as events from long-running commands.
- **Payload.** Each event includes duration, outcome, and invocation environment; events for invocations with a resolvable graph ref include the graph identifier, and others omit it. A command that panics reports an internal error.
- **Graph identifier.** Hashing the set of known graph refs does not reproduce any emitted graph identifier.
- **Compatibility.** Setting the `APOLLO_TELEMETRY_DISABLED` environment variable to any value results in no event, as today.

---

## Rollout

- **Slice one: disclosure and minimization.** Correction of the privacy documentation's exclusion claim and removal of unscrubbed argument values. Smallest change, and it repairs the existing trust gap. The documentation fix is its own PR, separate from code changes.
- **Slice two: exit delay.** Lower the request timeout while keeping the wait.
- **Slice three: payload.** Duration, invocation environment, graph identifier (once its derivation is settled), and outcome classification, with their disclosure. Outcome classification may split further, since it needs the error-mapping decision and panic handling.
- Each shipped slice is a user-visible change and gets a changelog entry.

## Related

- An earlier internal proposal for identified, account-linked usage tracking, which this PRD rejects in favor of anonymous telemetry.
- Earlier internal proposals for duration, outcome, and feature-usage fields, largely unimplemented.
