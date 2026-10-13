# PRD: Rover Telemetry

- **Owner:** Brian George
- **Component:** Rover CLI (anonymous usage telemetry and request tracing)

This PRD reviews Rover's current usage telemetry, benchmarks it against other industry CLI applications, and recommends where Rover should land. It has two parts that share a problem statement but ship independently. **Part A** (usage telemetry) fixes disclosure and data-minimization gaps and adds the fields needed for aggregate analysis. **Part B** (request tracing) lets Platform API failures be root-caused end to end. Neither part introduces a persistent, account-linked user identity.

One decision shapes both parts: **telemetry stays anonymous.** No other industry CLI we surveyed ties usage telemetry to authenticated account identity. If we want user-to-platform usage correlation, server-side capabilities should achieve it without Rover sending identity.

## 1. Problem statement

Rover's telemetry is a single fire-and-report request per invocation. Measured against its own published privacy documentation and against other industry CLI applications, it has five concrete problems:

1. **The published privacy documentation is inaccurate.** The privacy documentation does not match what Rover actually collects: some of its claims about what is excluded do not hold, and some collected fields are not disclosed at all.
2. **Telemetry can delay every command.** Reporting runs in the background, but the process always waits for it before exiting. On a slow or unreachable network, every command can be delayed by up to the full request timeout, contradicting the intent that telemetry is not in the critical path.
3. **The payload is too thin to be useful for product decisions.** There is no command duration, no success-or-error outcome, and no way to group invocations by graph. A 2024 internal proposal asked for duration, outcome, and derived feature-usage fields; none were built.
4. **No enterprise or regulated-customer exemption.** Telemetry is sent regardless of where the user is authenticated. At least one widely used industry CLI skips telemetry entirely when authenticated against a self-hosted enterprise server.
5. **Platform API failures can't be root-caused from the client side.** Customers have reported a variety of errors coming back through Rover's error chain when running `rover supergraph compose`. Retries were added as a band-aid, but the underlying problems appear to originate in the Platform API, and diagnosing them requires request-level data Rover does not send.

## 2. Background: how this works today

- **One report per invocation.** When enabled, Rover sends one request to Apollo's telemetry endpoint per invocation, with a 1-second timeout, no batching, and **no sampling**. Debug builds never report.
- **Opt-out only.** Telemetry is disabled by the `APOLLO_TELEMETRY_DISABLED` environment variable, which is presence-based: any value, including `false`, disables it. There is no flag and no persisted setting.
- **What is sent.** Command name and all of its arguments with their raw values; a persistent anonymous machine identifier; a per-invocation session identifier; a SHA-256 hash of the working directory; a SHA-256 hash of the Git remote URL when one exists; OS, architecture, and CI vendor when detected; and the Rover version.
- **What is not sent.** Duration, outcome (success or error), derived feature usage, or any account, org, or graph identifier.
- **Telemetry is disconnected from authentication.** No identity plumbing exists between telemetry and Rover's auth subsystem, even experimentally.
- **Error codes are not classified.** Rover's error codes are a flat list with no notion of user-caused versus internal or platform-caused errors. Rust panics bypass the error-code path entirely.
- **Request tracing is absent.** Rover sends no W3C Trace Context headers on Platform API requests.

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

---

# Part A: Usage telemetry

## A1. Goals

1. **Accurate disclosure.** Every field Rover sends is documented in the privacy documentation, including the Git remote URL hash. A reader can compare the docs to the payload and find no discrepancy.
2. **No raw argument values.** Rover reports flag names only, plus the values of allowlisted flags. No identifying value is sent. This makes the privacy documentation's existing claim true.
3. **Bounded exit delay.** Telemetry never delays process exit by more than a small, fixed cap on a slow or unreachable network, and still delivers reliably in the normal case where the request completes in tens of milliseconds. Telemetry must not silently drop events for commands that finish in a few milliseconds.
4. **Duration.** Each event includes the wall-clock duration of the command's execution.
5. **Graph identifier.** When a graph ref is resolvable for the invocation, the event includes a SHA-256 hash of it, using the same hashing convention as the existing working-directory and remote-URL hashes. When no graph ref is resolvable, the field is absent. The hash is disclosed in the privacy documentation alongside the others.
6. **Outcome classification.** Each event includes the command's outcome: success, user error, or internal error. Rust panics are classified as internal errors.
7. **Full backward compatibility for opt-out.** `APOLLO_TELEMETRY_DISABLED` continues to disable telemetry exactly as it does today. Everything added is additive.

## A2. Key decisions and rationale

- **Shrink the exit delay; don't remove it.** Fully detaching the reporting task risks the process exiting before a fast command's request has even connected. For commands that finish in milliseconds, which is likely faster than name resolution plus a TLS handshake, that would silently drop events, which is worse than the current delay. Keeping the wait but lowering the request timeout well below one second (candidate range 150 to 300 milliseconds) caps the worst-case cost on a bad network at something barely noticeable while keeping delivery reliable in the normal case.
- **Flag names by default, values by allowlist.** This matches the strictest posture among the CLIs surveyed and is the only one that can be shown correct without auditing every flag's value space. A flag whose values are provably non-identifying can opt in by name. Everything else, including any new flag, is name-only by default, so a new flag can never leak a value by omission.
- **The graph identifier is resource-scoped, not identity-scoped.** It carries no more identifying weight than the hashes Rover already sends, and it is the one new field needed to aggregate usage by graph cohort. Cohort aggregation is the substitute for account-linked identity (see the Non-goals).
- **Duration and outcome have direct industry precedent.** At least one surveyed CLI sends both; neither requires identity.
- **User error versus internal error is new classification work.** No existing error code is marked as user-caused or internal. The mapping has to be decided for every error, and panics need a separate path because they never become an error code.
- **Anonymity is preserved.** No field in this part ties an event to an account, org, or person.

## A3. Non-goals

- A persistent cross-session user identifier (`user_id`). Dropped this round; reconsider only if cohort aggregation is tried and found genuinely insufficient.
- Account- or org-level fields.
- Capturing responses to interactive prompts.
- Changing the default (on, opt-out) or adding an opt-in prompt.
- Sampling.
- Enterprise or regulated-customer exemption. One surveyed CLI skips telemetry against a self-hosted enterprise server, but that depends on how Rover would identify such an environment, so it is an open question (A4) rather than a goal.

## A4. Open questions (resolve in the Part A spec)

- **Timeout value.** The exact cap on the telemetry request, within the 150 to 300 millisecond candidate range.
- **Allowlist contents.** Which flags have values provably non-identifying enough to report.
- **Error classification.** Which error codes are user errors and which are internal, and what happens to errors that are plausibly both.
- **Enterprise exemption.** Whether Rover should skip telemetry for non-default registries or enterprise environments, and how it would know.
- **Boolean semantics of `APOLLO_TELEMETRY_DISABLED`.** Whether to keep it presence-only, which is the current behavior and a quirk, or normalize it. Settled alongside the profile configuration work.
- **Sampling.** Whether reporting 100% of invocations is still justified once duration and outcome make events more useful, given one surveyed CLI samples about 1%.

## A5. Success metrics

- **Disclosure.** Every field in the payload appears in the privacy documentation, and every field described there is in the payload. The Git remote URL hash and the graph hash are both listed.
- **Minimization.** For any invocation, no flag value outside the allowlist appears in the event, verified against the full set of global and per-command flags.
- **Exit delay.** On an unreachable telemetry endpoint, no command's exit is delayed by more than the chosen cap.
- **Delivery.** On a reachable endpoint, events from commands that finish in a few milliseconds are delivered at the same rate as events from long-running commands.
- **Payload.** Each event includes duration and outcome; events for invocations with a resolvable graph ref include the graph hash, and others omit it. A command that panics reports an internal error.
- **Compatibility.** Setting `APOLLO_TELEMETRY_DISABLED` to any value results in no event, as today.

---

# Part B: Request tracing

Independent of Part A and of its anonymity discussion. It addresses reliability root-causing, not aggregate behavior.

## B1. Goals

1. **Trace context on Platform API requests.** Rover sends the W3C Trace Context headers (`traceparent` and `tracestate`) on requests to the Platform API, so a failure, or a set of failures, can be followed across every service involved in the request.
2. **Anonymous request-level data.** `tracestate` carries request-level data such as the number of retries attempted for that request, giving visibility into how often and where requests fail. It never carries anything user-identifiable.
3. **Platform-side support.** The Platform API gateway parses both headers, uses `traceparent` as the trace ID for subsequent logs and traces, forwards the data to its observability backends, and propagates the headers to internal services.

## B2. Key decisions and rationale

- **A standard, not a bespoke header.** W3C Trace Context is understood by observability tooling on the platform side, so the gateway doesn't need Rover-specific parsing.
- **Anonymous by construction.** `tracestate` carries request-level facts only. This keeps tracing outside the anonymity decision at the top of this PRD.
- **Ownership is split.** The Rover team owns the Rover-side changes. The Platform API team owns gateway parsing, forwarding to observability backends, and propagation to internal services. Part B has no user-visible value until the gateway side lands.
- **Future extension, not committed.** If users separately opt in, the anonymous usage data Rover already collects could ride alongside this channel for a more comprehensive field view. This is orthogonal to Part A and not part of this PRD.

## B3. Non-goals

- Account, org, or user identifiers in either header.
- Gateway-side implementation; this PRD records the dependency but the Platform API team owns the work.
- Tracing requests to destinations other than the Platform API.

## B4. Open questions (resolve in the Part B spec)

- **Owning team and sequencing** for the gateway work, and whether Rover ships headers before the gateway consumes them.
- **Header scope.** Whether every Platform API request carries them or only selected operations.
- **Privacy documentation.** Whether and how the headers are disclosed, given they are not part of the telemetry event.

## B5. Success metrics

- **Propagation.** A request from Rover to the Platform API carries both headers, and the gateway's logs for that request carry the same trace ID.
- **Retry visibility.** For a request that Rover retries, the retry count is visible on the platform side for the final attempt.
- **Anonymity.** No header value contains an identifier tied to a person, account, or organization.

---

## Rollout

- **Slice one (Part A): disclosure and minimization.** Privacy documentation corrections and removal of raw argument values. Smallest change, and it repairs the existing trust gap. The documentation fix is its own PR, separate from code changes.
- **Slice two (Part A): exit delay.** Lower the request timeout while keeping the wait.
- **Slice three (Part A): payload.** Duration, graph hash, and outcome classification, with their disclosure. Outcome classification may split further, since it needs the error-mapping decision and panic handling.
- **Part B.** Its own spec and slice, sequenced with the Platform API team.
- Each shipped slice is a user-visible change and gets a changelog entry.

## Related

- 2024-07-19 "Rover (or any other platform-api client) Telemetry": the original motivation and ownership breakdown for Part B.
- 2024-08-23 "Expanding Rover Telemetry & Metrics": the original duration, outcome, and feature-usage proposal, largely unimplemented.
- JPD-632 and its linked "Proposed" PRD: the identified-tracking direction this PRD responds to.
- Source: the 2026-08-03 Confluence page "Rover Telemetry: Current State, Industry Comparison, and Recommendation" (Runtime Readiness space).
