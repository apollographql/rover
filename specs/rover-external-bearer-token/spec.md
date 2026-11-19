# Spec: Application-supplied Platform API bearer tokens

- **PRD:** [prd.md](./prd.md)

## 1. Purpose and definitions

This specification defines how Rover consumes an Apollo OAuth access token owned by another application. It describes observable behavior, not implementation.

- **External token source:** The application-managed file whose location is given by the global `--access-token-file` option or the `APOLLO_ACCESS_TOKEN_FILE` environment variable.
- **External token:** The current OAuth access token read from that source.
- **Platform API endpoint:** The endpoint Rover is configured to use for Platform API operations, including an explicit endpoint override.
- **Attempt:** One outgoing HTTP send for a Platform API operation. A retry is another attempt. Each poll is a new operation.
- **Credential rejection:** A response Rover recognizes as refusing authentication. An authorization failure for an authenticated identity is not a credential rejection. A *status rejection* is a credential rejection signalled by an HTTP 401 or 406 response status, the statuses Rover already treats as credential rejections; an HTTP 403 is a permission denial, not a credential rejection. Any other credential rejection, such as an authentication error reported in the body of a successful response, is a *body rejection*.
- **Source failure:** An invalid source setting, an unsuccessful read, a source refused by the token file checks, or file contents that do not satisfy the token format.
- **Token file checks:** The safety checks in FR32 to FR35 that Rover applies to the source on every read.

## 2. Functional requirements

### 2.1 Input and selection

- **FR1:** Rover must accept the location of the external token source as the global `--access-token-file <PATH>` option, available on every command like Rover's other global options, or as the `APOLLO_ACCESS_TOKEN_FILE` environment variable. The value is a file location, never a token.
- **FR2:** When both the `--access-token-file` option and the `APOLLO_ACCESS_TOKEN_FILE` variable are present, the option takes precedence over the variable, as it does for Rover's other global options that have an environment equivalent.
- **FR3:** This feature must not add a token-value command-line option or allow the location to be stored in a profile or project configuration.
- **FR4:** A present but empty location, whether from `--access-token-file` or `APOLLO_ACCESS_TOKEN_FILE`, must be a source failure, not equivalent to an absent setting.
- **FR5:** A relative location must remain relative to the invocation's initial working directory for the lifetime of the process.
- **FR6:** When external authentication is selected, Rover must not require a stored profile credential. Non-credential settings from the selected profile must retain their existing behavior.
- **FR7:** When authentication is needed, a present token-source location, from either `--access-token-file` or `APOLLO_ACCESS_TOKEN_FILE`, combined with a non-empty `APOLLO_KEY` or any present `APOLLO_CLIENT_ID` or `APOLLO_CLIENT_SECRET` must fail as conflicting authentication inputs before a Platform API request or client-credentials exchange is made. An empty `APOLLO_KEY` is treated as absent. The error must identify the conflicting option and variable names, not their values. This applies at the first authenticated operation, however late in the command that occurs.
- **FR8:** When a command that can authenticate starts with a present token-source location and a conflicting credential input as defined in FR7, Rover must warn, naming the option and variable names but not their values, without failing the command. A command that needs no authentication must not warn.
- **FR9:** In `rover dev`, a non-empty `APOLLO_KEY` is not a conflicting authentication input. Rover must authenticate its own Platform API requests with the external token, and must make `APOLLO_KEY` available to the router it starts as an environment variable exactly as it does without an external token source. Rover must not use `APOLLO_KEY` to authenticate its own requests while an external token source is present, and must not warn about it as a conflict. When `APOLLO_KEY` is not set, Rover must not give the router a credential from the selected profile or from the external token: the router runs without GraphOS credentials, and Rover reports that as it does today when none are available.
- **FR10:** When neither `--access-token-file` nor `APOLLO_ACCESS_TOKEN_FILE` is present, existing credential selection and authentication behavior must remain unchanged.
- **FR11:** Commands that need no authentication must not read the external token source or fail because it is unavailable or conflicts with another credential input.

### 2.2 Token format and use

- **FR12:** The source must contain one raw, non-empty UTF-8 bearer token made only of the characters of an RFC 6750 `b64token`: one or more letters, digits, or `-`, `.`, `_`, `~`, `+`, `/`, followed by any number of `=`. Rover must accept either no final line ending, one final LF, or one final CRLF. It must reject other whitespace, additional lines, an `Authorization` header or `Bearer` prefix, a byte order mark, and any other character.
- **FR13:** Rover must not parse, validate, or require claims in a JWT-shaped token. It must send a token that satisfies FR12 unchanged, including a JWT that has expired.
- **FR14:** Every attempt authenticated with an external token must send that token as `Authorization: Bearer`. It must not also send an API-key credential or substitute a profile credential.
- **FR15:** Rover must read the configured source anew before every Platform API attempt, including the first attempt, eligible retries, subsequent operations, and polling requests. This must hold for every Platform API request path.
- **FR16:** An atomic replacement completed before an attempt's source read must supply that attempt's token. A replacement concurrent with a read may supply the complete old token or complete new token. Concurrent requests must each obtain a complete token without changing another in-flight request's credential. Rover cannot detect a token truncated by an application that rewrites the source in place: if the partial contents still satisfy FR12, Rover sends them and the outcome is a rejection handled under FR21.
- **FR17:** After a source failure, Rover must not send the affected attempt, reuse an earlier token, use another credential, or initiate an interactive login. Its diagnostic must identify which of `--access-token-file` or `APOLLO_ACCESS_TOKEN_FILE` supplied the location, and the failure category, without revealing file contents.
- **FR18:** Rover must not attach the external token to requests other than Platform API requests, including introspection, plugin downloads, OAuth endpoints, and telemetry. It must not explicitly forward the token value to unrelated subprocesses. The `APOLLO_ACCESS_TOKEN_FILE` variable is a location, not a secret, and Rover need not remove it from the environment of its subprocesses.

### 2.3 Ownership and failures

- **FR19:** Rover must not create, modify, delete, or persist a copy of the external token source or its token. It must not acquire or use a refresh token for this credential, refresh it, or revoke it. The integrating application owns source security (see its [contract](./contract.md)), atomic replacement, refresh, and cleanup, and is responsible for the token representing one account for the whole life of a Rover process.
- **FR20:** A body rejection must fail the affected operation without automatic replay.
- **FR21:** After a status rejection, Rover must read the source once more. If that read succeeds and yields a token different from the rejected one, Rover must resend the rejected request once with the new token. Rover must fail the operation if the token is unchanged, the read is a source failure, or the resent request is also rejected. Rover must not wait for the source to change, and must make at most one such resend per operation. The resend is safe only because the Platform API rejects an unauthenticated request before doing any work; this requirement depends on that behavior.
- **FR22:** When an operation fails because of a credential rejection, Rover must explain that the application-supplied credential was rejected and direct the caller to renew it in the supplying application, not to log Rover in.
- **FR23:** Apart from the single resend in FR21, external authentication must not add retries or change which transient failures are eligible for the command's existing retries. Each eligible retry must reread the source. A source failure must not itself trigger an automatic retry. The resend counts as an attempt of the same operation wherever Rover counts attempts, such as the retry count in its request trace headers.
- **FR24:** An authenticated permission denial must retain permission-denied behavior. It must not trigger token refresh, credential fallback, or an authentication retry.
- **FR25:** This feature must not introduce a new command recovery policy. Where a long-running command already permits later independent operations after an error, each later operation must read the source anew and may use a replacement supplied after the error.
- **FR26:** In JSON output, each of the following must have its own stable `error.code`, so that a calling application can tell them apart without parsing messages: a source failure, a credential rejection, conflicting authentication inputs, and a permission denial. The kinds of source failure (the location is empty, the source is missing, unreadable, or larger than FR34 allows, has contents that do not satisfy FR12, or is refused by another token file check) must also have distinct codes.

### 2.4 Identity, privacy, and profile management

- **FR27:** `rover auth whoami` and `rover config whoami` must report the external token source as the active credential origin, naming which of `--access-token-file` or `APOLLO_ACCESS_TOKEN_FILE` supplied it, authenticate their Platform API identity lookup with the current external token, and work without a stored profile credential. They must not infer or claim the OAuth grant type that produced an external token, or require an OAuth userinfo request.
- **FR28:** The identity of a request is whatever the token supplied for that attempt represents. Rover must not pin, record, or compare the identity across attempts, and must not treat a replacement token for a different account as an error. Two identity lookups in one process, or operations in a long-running command such as `rover dev`, may therefore resolve to different accounts if the integrating application replaces the token between them.
- **FR29:** Identity output must redact the token by default. Existing deliberate credential-unmasking options may expose it only when explicitly requested. Logs and errors, including debug output, must not reveal token contents. Telemetry must contain neither token contents nor the token-source location, including the value given to `--access-token-file` among a command's recorded arguments. Output shown to the user, including diagnostics, may include the resolved token-source location.
- **FR30:** `rover auth login` and `rover auth logout` must retain their selected-profile behavior independently of the external source. They must not read the external source, reject conflicting external-authentication inputs, change the source, or revoke its token merely because `--access-token-file` or `APOLLO_ACCESS_TOKEN_FILE` is present.
- **FR31:** Identity output and errors must use Rover's existing text and JSON output conventions. JSON output must retain the standard `json_version`, `data`, and `error` envelope.

### 2.5 Token file checks

- **FR32:** On every read, Rover must refuse the source if its configured location is itself a symbolic link. Symbolic links among the directories above it must be accepted.
- **FR33:** Where the filesystem enforces Unix file permissions, Rover must refuse the source if it is not owned by the user running Rover, or if users other than its owner can read or write it. Rover must also refuse it if the directory that holds it is owned by neither the user running Rover nor the system administrator, or can be written to by its group or other users. A directory's sticky bit does not make a group- or world-writable directory acceptable, which is how OpenSSH's `StrictModes` checks a user's key files.
- **FR34:** Rover must refuse a source larger than 16 KiB, and must not read more than 16 KiB plus one byte from it, the extra byte being what shows that the source is too large, including one that grew after it was checked. A source of exactly 16 KiB (16,384 bytes) is not refused for its size.
- **FR35:** The file that Rover checks must be the file it reads. Rover must not check one file and then read a different file because the source changed in between, and must not use contents that were not subject to the checks.
- **FR36:** Where the filesystem does not enforce Unix file permissions, including on Windows, Rover must apply FR32, FR34, and FR35 but not FR33.
- **FR37:** A refused source is a source failure under FR17. Its diagnostic must name the check that failed and how to fix it, without revealing the file's contents.

## 3. Acceptance criteria

### No separate Rover login

**Given** a valid external token source and no stored Rover credentials, **when** an authenticated Platform API command runs, **then** its requests use the external bearer token without a login prompt or credential-store write.

### Existing authentication remains available

**Given** neither `--access-token-file` nor `APOLLO_ACCESS_TOKEN_FILE`, **when** commands use an API key, client credentials, or a stored OAuth login, **then** credential selection and behavior remain unchanged.

### Conflicting runtime credentials

**Given** an external source, supplied by either the option or the variable, and any of a non-empty `APOLLO_KEY`, `APOLLO_CLIENT_ID`, or `APOLLO_CLIENT_SECRET`, including a partial client-credentials pair, **when** an authenticated operation starts, **then** it fails with the conflicting option and variable names, without a token exchange, Platform API request, or disclosure of their values.

**Given** an external source and an empty `APOLLO_KEY`, **when** an authenticated command runs, **then** the empty variable is ignored and the command uses the external token.

**Given** an external source and a conflicting credential input, **when** a command that can authenticate starts, **then** it warns about the conflict, naming but not revealing the inputs, and keeps running until an authenticated operation needs the credential.

**Given** a long-running command that makes no authenticated request until some time after it starts, **when** the first authenticated operation begins with a conflicting credential input, **then** the command fails at that point with the conflict error.

### `rover dev` and the router's `APOLLO_KEY`

**Given** an external source and a non-empty `APOLLO_KEY`, **when** `rover dev` runs, **then** it makes no conflict error and no conflict warning, authenticates its own Platform API requests with the external token, and passes `APOLLO_KEY` to the router it starts as an environment variable.

**Given** an external source, no `APOLLO_KEY`, and a selected profile with a stored API key, **when** `rover dev` starts the router, **then** the router receives no `APOLLO_KEY` from the profile or the token, and Rover reports that it is running without GraphOS credentials.

### Profile settings remain effective

**Given** a selected profile containing settings and a different stored credential, **when** an operation uses an external token, **then** applicable profile settings still apply but the stored credential is not used.

### Option and environment variable

**Given** a valid token source whose location is given only by `--access-token-file`, **when** an authenticated command runs, **then** its requests use that token, with or without `APOLLO_ACCESS_TOKEN_FILE` being set.

**Given** a valid token source whose location is given only by `APOLLO_ACCESS_TOKEN_FILE`, **when** an authenticated command runs, **then** its requests use that token.

**Given** both settings naming different files, **when** an authenticated command runs, **then** Rover reads only the file named by `--access-token-file`, and a failure in the file named by the variable does not affect the command.

**Given** `--access-token-file` with an empty value, **when** an authenticated command runs, **then** the attempt is not sent and the diagnostic names `--access-token-file`, even if `APOLLO_ACCESS_TOKEN_FILE` names a valid source.

**Given** the option on a command that needs no authentication, **when** it runs, **then** the option is accepted, the source is not read, and the command does not fail.

### Input format

**Given** an opaque, non-JWT bearer token, **when** the file contains that token with no terminator, one LF, or one CRLF, **then** Rover sends the same token in all three cases.

**Given** an empty file, invalid UTF-8, whitespace inside a token, extra lines, a prefixed header value, a byte order mark, or an invalid bearer-token character, **when** Rover needs the token, **then** the affected attempt is not sent and the diagnostic does not repeat the contents.

**Given** a JWT-shaped token that has expired, **when** Rover needs the token, **then** it sends the token unchanged and does not fail because of its claims.

### Rotation without restart

**Given** a running command that has used token A, **when** the application atomically replaces the source with token B before the next attempt's read, **then** that attempt uses token B without restarting Rover.

### Rotation during an eligible retry

**Given** a transient failure eligible for an existing retry and token A on the failed attempt, **when** the source is replaced with token B before the retry reads it, **then** the retry uses token B without adding an authentication-specific attempt.

### Concurrent requests and replacement

**Given** concurrent Platform API attempts and an atomic token replacement, **when** source reads overlap the replacement, **then** every request uses a complete token A or complete token B, and replacing the source does not mutate headers on a request already in flight.

### Token rewritten in place

**Given** an application that rewrites the source in place, **when** Rover reads a truncated token that still satisfies the token format, **then** Rover sends it and, on a status rejection, handles it as in the rejected-credentials criteria rather than as a source failure.

### Source failure after a successful request

**Given** an attempt that succeeded with token A, **when** the source is removed, becomes unreadable, becomes invalid, or fails a token file check before the next source read, **then** the next attempt is not sent and Rover does not reuse token A or a saved credential.

### Source repair in a continuing command

**Given** a source failure in a command whose existing policy permits later independent operations, **when** the application repairs the source before the next independent operation, **then** that operation can authenticate with the repaired source without restarting Rover.

### Rejected credentials and permissions

**Given** an attempt with token A that receives a status rejection (HTTP 401 or 406), **when** the source holds token B by the time Rover rereads it, **then** Rover resends the rejected request once with token B, and the command continues without a restart.

**Given** an attempt with token A that receives a status rejection, **when** the source still holds token A, **then** the operation fails at once with the rejection, without a resend and without waiting for the source to change.

**Given** an attempt with token A that receives a status rejection and a resend with token B that also receives a status rejection, **when** Rover handles the second rejection, **then** the operation fails without a further attempt.

**Given** an attempt that receives a status rejection, **when** rereading the source is a source failure, **then** the operation fails without a resend.

**Given** a body rejection, **when** a new token subsequently appears, **then** Rover reports the rejection without automatically replaying the rejected operation.

**Given** a credential rejection that fails the operation, **when** Rover reports it, **then** the message says the application-supplied credential was rejected and directs the caller to renew it in the supplying application, not to log Rover in.

**Given** an authenticated identity without permission, **when** the Platform API denies the operation with an HTTP 403, **then** Rover reports a permission failure without changing credentials or initiating refresh.

### Error codes

**Given** JSON output, **when** a source failure, a credential rejection, conflicting authentication inputs, and a permission denial each occur, **then** each reports a different stable `error.code`, and the kinds of source failure report different codes from one another.

### Token file checks

**Given** a source location that is a symbolic link to an otherwise valid token file, **when** Rover needs the token, **then** the attempt is not sent and the diagnostic says the location is a symbolic link.

**Given** a valid token file in a directory reached through a symbolic link, **when** Rover needs the token, **then** it uses the token.

**Given** a filesystem that enforces Unix file permissions and a token file that other users can read, that another user owns, or whose directory is owned by another user (other than the system administrator) or is writable by its group or other users, **when** Rover needs the token, **then** the attempt is not sent and the diagnostic names the failed check and how to fix it.

**Given** a token file directly inside a world-writable directory with the sticky bit set, such as `/tmp`, **when** Rover needs the token, **then** the attempt is not sent and the diagnostic names the directory check, as OpenSSH's `StrictModes` would for a key file there.

**Given** a token file larger than 16 KiB, **when** Rover needs the token, **then** the attempt is not sent and Rover reads no more than 16 KiB plus one byte of it.

**Given** a token file of exactly 16,384 bytes whose contents otherwise satisfy the token format, **when** Rover needs the token, **then** it is not refused for its size.

**Given** the file at the source location changes between the checks and the read, **when** Rover needs the token, **then** the contents it uses are those of the file that passed the checks.

**Given** a filesystem that does not enforce Unix file permissions, including Windows, and a token file that would fail only the Unix permission checks, **when** Rover needs the token, **then** it uses the token.

### Identity reporting

**Given** an external source and no stored credential, **when** either identity command runs, **then** it uses the current token for a Platform API identity lookup, reports the external token source as the origin, naming the option or variable that supplied it, makes no userinfo request, and does not claim a known grant type. Text and JSON redact the token unless the caller explicitly requests unmasking; JSON retains the standard envelope.

**Given** a long-running command that has used a token for one account, **when** the application replaces the source with a token for a different account, **then** later operations use the new token without an error, and an identity lookup may report a different identity than an earlier one.

### Destination isolation and diagnostics

**Given** external authentication, **when** Rover makes Platform API requests and requests to unrelated destinations, **then** only Platform API requests carry the external token.

**Given** successful and failing source reads, malformed contents, and rejected tokens, **when** output, debug logs, and telemetry are inspected, **then** none exposes token contents except explicitly requested identity unmasking, and telemetry does not include the token-source location, including the value of `--access-token-file`.

### Local and profile-management commands

**Given** an unavailable external source and conflicting runtime credential variables, **when** a command that needs no authentication runs, **then** external authentication does not cause it to fail, warn, or read the source.

**Given** the same external-authentication settings, **when** `rover auth login` or `rover auth logout` runs, **then** it retains its selected-profile behavior without reading or modifying the external source or revoking the external token.

### Stable source location

**Given** a relative source location from either the option or the variable, **when** Rover performs later operations, **then** it keeps resolving that location from the invocation's initial working directory rather than changing its meaning during the command.

## 4. Non-goals

- A second login, refresh-token sharing, token exchange, or grant management.
- A raw-token environment variable or token-value command-line option.
- A credential helper, local IPC protocol, or background refresh service.
- Authenticating requests outside the configured Platform API endpoint.
- Making an insecurely managed source safe beyond the token file checks, including checking Windows access-control lists.
- Changing existing command recovery policies or replaying rejected operations beyond the single resend after a status rejection.
- Detecting, preventing, or pinning a change of the account a token represents during a Rover process.
- Detecting a token that an application truncated by writing in place.
