# Spec: Rover telemetry behavior

- **PRD:** [prd.md](./prd.md)

## 1. Purpose and scope

This spec defines the required behavior of Rover's anonymous usage telemetry, per PRD goals 1 through 8. It describes what the system must do and the exact conditions involved, not how it's implemented. Implementation approach belongs in a separate design/implementation plan. Each requirement notes the PRD goal it serves.

Request tracing against the Platform API is out of scope; it has its own PRD.

## 2. Terminology

- **Telemetry event**: the single anonymous report Rover sends for one invocation.
- **Baseline fields**: the fields an event contains today, other than argument fields: the command name, the persistent anonymous machine identifier, the per-invocation session identifier, the working-directory hash, the Git-remote hash, the operating system, the CPU architecture, the CI vendor, and the Rover version.
- **Identifying value**: a flag or argument value that could identify a person, organization, machine, or project, such as file-system paths, profile names, URLs, graph refs, keys, and free-text values.
- **Resource-scoped identifier**: a one-way identifier of something a user works on (a graph, a working directory, a repository), as opposed to one that names a person or account. Resource-scoped identifiers are not identifying values.
- **Supplied flag**: a flag given on the command line. Its value may instead reach the command through an environment variable or configuration; such a value is never reported (FR9).
- **Allowlisted flag**: a flag on an explicit allowlist because every value it can take is provably non-identifying. Only a boolean flag or a flag whose values come from a fixed set (for example, `--format`) is eligible.
- **Delivered**: the event's request was fully sent to the telemetry endpoint. Acknowledgement isn't required.
- **Cancelled invocation**: an invocation whose command was stopped by `SIGTERM` or `SIGINT` before it completed on its own.
- **Outcome**: one of `success`, `user_error`, `internal_error`, `connection_error`, or `terminated`.
- **Invocation environment**: a coarse classification of the context an invocation ran in, with exactly two values: `automated` (CI is detected, or standard input isn't a terminal) and `interactive` (everything else, including a terminal user whose output is piped).
- **Resolvable graph ref**: a well-formed graph ref that the command obtains from a flag, an environment variable, a profile, or project configuration.
- **Install salt**: a random value generated on an install's first use and stored beside the machine identifier. It is never sent.
- **Graph identifier**: a one-way identifier derived from an invocation's resolvable graph ref and the install salt.

## 3. Functional requirements

### 3.1 Disclosure

- **FR1** (Goal 1): For every released version, the "Privacy and Data Collection" documentation must list every field that version's event contains, baseline and added.
- **FR2** (Goal 1): The documentation must not state that any value, flag, or category is excluded from telemetry unless the event honors that exclusion for every command.
- **FR3** (Goal 1): Documentation may be published before the release that adds a field. A field described ahead of its release must state the Rover version in which it first appears. The parity requirement (FR1) applies to released versions, not to the documentation at an arbitrary point between releases.

### 3.2 Baseline fields

- **FR4** (Goal 8): Rover must continue to send the baseline fields with their current meaning. The machine identifier's handling is out of scope for this spec and is unchanged. The working-directory and Git-remote hashes are resource-scoped identifiers and are unaffected by FR10.

### 3.3 Argument minimization

- **FR5** (Goal 2): A telemetry event must report the names of the supplied flags, never their values, except as FR6 allows. A flag given in a short form or by an alias is reported under its canonical long name.
- **FR6** (Goal 2): A telemetry event may report the value of a supplied flag only if that flag is allowlisted.
- **FR7** (Goal 2): A flag that isn't allowlisted, including any flag added in the future, must be reported by name only.
- **FR8** (Goal 2): Positional arguments must not be reported, neither by value nor by placeholder. The command name, including its subcommands, is reported as today.
- **FR9** (Goal 2): A flag's value must not be reported when it reached the command through an environment variable or configuration rather than the command line, even for an allowlisted flag.
- **FR10** (Goal 2): No argument or flag value that is an identifying value may appear in a telemetry event.
- **FR11** (Goal 2): The allowlist may be empty. A flag that is neither boolean nor takes a fixed set of values must not be allowlisted. A test must enumerate every global and per-command flag and fail when any flag has not been explicitly classified as name-only or allowlisted, or when an allowlisted flag isn't eligible, so that adding a flag forces the decision.

### 3.4 Exit delay

- **FR12** (Goal 3): Rover must send exactly one event per invocation, after the command completes. After a command completes, Rover must delay process exit because of telemetry by no more than 250 milliseconds in total, however slow or unreachable the telemetry endpoint is. The budget covers success, error, panic, and termination-signal paths and is not additive across them. A process that is killed without a chance to run cleanup sends no event.
- **FR13** (Goal 3): When the telemetry endpoint is reachable and accepts the request within that budget, the event must be delivered even if the command finished in a few milliseconds. A short-running command must not lose its event because the process exited first.
- **FR14** (Goal 3): A telemetry failure or timeout must not change the command's output or exit code, and must print nothing at the default log level. It may be logged at debug level.
- **FR14a** (Goal 3): Rover must run every command, together with its telemetry, under a harness that handles `SIGTERM` and `SIGINT` in the same way. When the process receives either signal while a command is running, the harness must stop the command, send the event, wait for telemetry no longer than the FR12 budget, and then exit. This applies to every command, including any that handle an interrupt themselves.
- **FR14b** (Goal 3): After handling a signal, Rover's exit status must be the one the process would have had if that signal had terminated it by default.
- **FR14c** (Goals 4, 6): An event for a cancelled invocation must omit `duration_ms`, because it would record how long the command ran before it was stopped, not how long it takes. It must report the outcome `terminated`, and must still include every other field, so general analytical data is kept.

### 3.5 Event payload

- **FR15** (Goal 4): Each event for a command that completes must include the wall-clock duration of the command's execution, as a whole number of milliseconds, measured from the start of command execution to its completion and excluding the telemetry wait. A cancelled invocation is governed by FR14c instead.
- **FR16** (Goal 6): Each event must include the command's outcome. The outcome is `success` when the command completes with exit code 0, `connection_error` when it fails because Rover couldn't reach a server or complete a network request, and `internal_error` when it fails for any other reason the user can't correct. It's `user_error` when it fails for a reason the user can correct, when it completes by design with a non-zero exit code (for example, a check that finds failures). A cancelled invocation reports `terminated` (FR14c), whichever signal stopped it. A command whose output pipe is closed by the reader reports `success`.
- **FR16a** (Goal 6): Reasons the user can correct include bad input, configuration, authentication, and schema or composition failures. Failures to reach a server, a dropped connection, and request timeouts are `connection_error`, whichever side caused them. For an HTTP error response, a 4xx status is `user_error` and a 5xx status is `internal_error`. Rover bugs and any other server error are `internal_error`.
- **FR17** (Goal 6): Every Rover error, including any added in the future, must map to exactly one of `user_error`, `internal_error`, or `connection_error`. An error that could plausibly be either, and any error with no explicit mapping, must map to `internal_error`. A test must enumerate every error and fail when one lacks an explicit mapping.
- **FR18** (Goal 6): A panic anywhere in command execution, on any thread, must result in `internal_error`, and the event must be sent before the process exits, subject to FR12. Release builds must unwind on panic so this is achievable. A panic while reporting must not change the command's output or exit code.
- **FR19** (Goals 3, 6): An invocation that exits before command execution, such as `--help`, `--version`, or an argument parsing failure, must send no event.
- **FR20** (Goal 7): Each event must include the invocation environment. It must not include hostnames, usernames, paths, or any other value that identifies a machine or person. The CI vendor stays a baseline field. Any invocation in which CI is detected, whether or not a vendor is identified, must report `automated`. An invocation whose standard input isn't a terminal must report `automated`.
- **FR21** (Goal 5): When the invocation has a resolvable graph ref, the event must include the graph identifier. When it has none, whether because no graph ref is supplied, the ref is malformed, or the command takes no graph ref, the field must be absent, not empty.
- **FR22** (Goal 5): The graph identifier must not be reproducible by hashing known graph refs. It must be derived from the graph ref and the install salt. The install salt must be random, generated per install, and never sent. A secret embedded in the Rover binary doesn't satisfy this, because it can be extracted.
- **FR22a** (Goal 5): For a given install, the same graph ref must yield the same graph identifier on every invocation, as long as the install salt is unchanged. Two installs must yield different identifiers for the same graph ref. The graph identifier therefore groups invocations from the same install only, not across users. Deleting the install salt resets the identifiers.
- **FR23** (Goal 8): An event must not contain any account, organization, or user identifier, and Rover must not send one.

### 3.6 Opt-out compatibility

- **FR24** (Goal 8): Setting the `APOLLO_TELEMETRY_DISABLED` environment variable to any value, including `false`, must continue to disable telemetry, so that no event is sent.
- **FR25** (Goal 8): The `--telemetry-disabled` flag, the profile setting, and the project setting must continue to behave as they do today.
- **FR26** (Goal 8): When telemetry is disabled, Rover must make no telemetry network request.
- **FR27** (Goal 8): Debug builds must continue to send no events.

### 3.7 Wire contract

- **FR28** (Goal 8): New fields must be additive and optional. Existing fields must keep their names and meaning, except that FR5 through FR10 narrow what the argument fields contain. The telemetry endpoint must accept events carrying the new fields before any Rover version that sends them is released, and must keep accepting events from older versions.
- **FR29** (Goals 4–7): The new fields are named `duration_ms`, `outcome`, `environment`, and `graph_id`. An absent value is encoded by omitting the field.

## 4. Acceptance criteria

**Flag values are not reported**
Given a command run with a non-allowlisted flag whose value is a file path, a URL, or a graph ref, when the event is built, then the event contains the flag's name and not its value.

**Allowlisted flag value**
Given a command run with `--format json` on the command line, when the event is built, then the event contains the flag `--format` with the value `json`.

**Values from environment or configuration**
Given an allowlisted flag whose value comes from an environment variable or configuration, when the event is built, then the event doesn't contain its value.

**Short forms and aliases**
Given a flag supplied in its short form, when the event is built, then the event reports its canonical long name.

**Positional arguments**
Given a command run with positional arguments, when the event is built, then neither their values nor placeholders for them appear in the event.

**New flags default to name-only**
Given a flag that was added after this spec and isn't on the allowlist, when a command uses it, then the event reports its name and not its value.

**Flag classification test**
Given a flag that hasn't been classified as name-only or allowlisted, when the flag-enumeration test runs, then it fails.

**Unreachable endpoint**
Given the telemetry endpoint accepts connections but never responds, when any command finishes, then the process exits no more than 250 milliseconds after the command completed, with the command's normal output and exit code.

**Fast command, reachable endpoint**
Given a reachable endpoint and a command that finishes in a few milliseconds, when the command completes, then the event is delivered.

**Duration and outcome on success**
Given a command that succeeds, when the event is built, then it includes `duration_ms` and the outcome `success`.

**Termination signal**
Given a running command and a reachable endpoint, when the process receives `SIGTERM` or `SIGINT`, then the command stops, an event is delivered that has no `duration_ms` and the outcome `terminated` but has the command name, flags, invocation environment, and baseline fields, and the process exits with the status a default termination by that signal would give.

**Termination signal, command that handles interrupts**
Given a command that handles an interrupt itself, when the process receives `SIGINT`, then the result is the same as for any other command: the command stops, the event has no `duration_ms` and the outcome `terminated`.

**Termination signal, unreachable endpoint**
Given a running command and an endpoint that never responds, when the process receives `SIGTERM` or `SIGINT`, then the process exits no more than 250 milliseconds after the command stopped.

**Failed check**
Given a command that completes by design with a non-zero exit code, when the event is built, then the outcome is `user_error`.

**User error**
Given a command that fails with an error mapped to `user_error`, when the event is built, then the outcome is `user_error`.

**Internal error**
Given a command that fails with an error mapped to `internal_error`, when the event is built, then the outcome is `internal_error`.

**Connection error**
Given a command that fails because the server is unreachable, the connection drops, or the request times out, when the event is built, then the outcome is `connection_error`.

**HTTP error status**
Given a command that fails on an HTTP error response, when the status is 4xx, then the outcome is `user_error`, and when it is 5xx, then the outcome is `internal_error`.

**Unmapped or ambiguous error**
Given an error with no explicit mapping, or one that could be either, when the event is built, then the outcome is `internal_error`.

**Error mapping test**
Given an error with no explicit mapping, when the error-enumeration test runs, then it fails.

**Panic**
Given a command that panics, when the process exits, then an event with the outcome `internal_error` has been delivered, subject to the FR12 budget, and the exit code is unchanged by telemetry.

**Automated environment**
Given a run in which a CI vendor is detected, when the event is built, then the invocation environment is `automated`.

**Interactive environment**
Given a run with standard input attached to a terminal and no CI detected, when the event is built, then the invocation environment is `interactive`, including when output is piped.

**Graph identifier present**
Given a command whose graph ref resolves, when the event is built, then it includes `graph_id`.

**Graph identifier stability and separation**
Given one install, when two invocations resolve the same graph ref, then their `graph_id` values are equal. Given two installs with different install salts, when each resolves the same graph ref, then their `graph_id` values differ.

**Help, version, and usage errors**
Given `--help`, `--version`, or an invalid argument, when the command exits, then no event is sent.

**Closed output pipe**
Given a command whose output reader closes the pipe, when the command exits, then the outcome is `success`.

**Generic CI and non-terminal input**
Given a run where CI is detected but no vendor is identified, or where standard input isn't a terminal, when the event is built, then the invocation environment is `automated`.

**Graph identifier absent**
Given a command with no resolvable graph ref, a malformed one, or no graph ref at all, when the event is built, then `graph_id` is absent.

**Graph identifier is not reproducible**
Given a list of known graph refs, when each is hashed with SHA-256, SHA-1, MD5, and BLAKE3, then no result equals an emitted `graph_id`.

**Opt-out**
Given `APOLLO_TELEMETRY_DISABLED` set to any value, `--telemetry-disabled`, or a profile or project setting that disables telemetry, when any command runs, then no event is sent and no telemetry network request is made.

**Debug builds**
Given a debug build, when any command runs, then no event is sent.

**Documentation parity**
Given a released Rover version, its privacy documentation, and a captured event from it, when each is compared with the other, then every field in the event is documented and every documented field is in the event.

**Endpoint compatibility**
Given the telemetry endpoint, when it receives an event from an older Rover version and one with the new fields, then it accepts both.

## 5. Non-goals

- A persistent cross-session user identifier.
- Account- or organization-level fields.
- Capturing responses to interactive prompts.
- Changing the default (on, opt-out) or adding an opt-in prompt.
- Changing the persistent machine identifier.
- Grouping invocations by graph across users.
- Request tracing against the Platform API.
- An enterprise or regulated-customer exemption, and sampling.
