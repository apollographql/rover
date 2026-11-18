# Spec: Application-supplied Platform API bearer tokens

- **PRD:** [prd.md](./prd.md)

## 1. Purpose and definitions

This specification defines how Rover consumes an Apollo OAuth access token owned by another application. It describes observable behavior, not implementation.

- **External token source:** The application-managed file whose location is given by the global `--access-token-file` option or the `APOLLO_ACCESS_TOKEN_FILE` environment variable.
- **External token:** The current OAuth access token read from that source.
- **Platform API endpoint:** The endpoint Rover is configured to use for Platform API operations, including an explicit endpoint override.
- **Attempt:** One outgoing HTTP send for a Platform API operation. A retry is another attempt. Each poll is a new operation.
- **Credential rejection:** A response Rover recognizes as refusing authentication. An authorization failure for an authenticated identity is not a credential rejection. A *status rejection* is a credential rejection signalled by an HTTP 401 response status. Any other credential rejection, such as one reported in the body of a successful response, is a *body rejection*.
- **Source failure:** An invalid source setting, an unsuccessful read, a source refused by the token file checks, or file contents that do not satisfy the token format.
- **Token file checks:** The safety checks in FR22 to FR26 that Rover applies to the source on every read.

## 2. Functional requirements

### 2.1 Input and selection

- **FR1:** Rover must accept the location of the external token source as the global `--access-token-file <PATH>` option, available on every command like Rover's other global options, or as the `APOLLO_ACCESS_TOKEN_FILE` environment variable. When both are present, the option takes precedence over the variable, as it does for Rover's other global options that have an environment equivalent. The option's value is a file location, never a token. This feature must not add a token-value command-line option or allow the setting to be stored in a profile or project configuration.
- **FR2:** A present but empty location, whether from `--access-token-file` or `APOLLO_ACCESS_TOKEN_FILE`, must be a source failure, not equivalent to an absent setting. A relative location must remain relative to the invocation's initial working directory for the lifetime of the process.
- **FR3:** When external authentication is selected, Rover must not require a stored profile credential. Non-credential settings from the selected profile must retain their existing behavior.
- **FR4:** When authentication is needed, a present token-source location, from either `--access-token-file` or `APOLLO_ACCESS_TOKEN_FILE`, combined with any present `APOLLO_KEY`, `APOLLO_CLIENT_ID`, or `APOLLO_CLIENT_SECRET` must fail as conflicting authentication inputs before a Platform API request or client-credentials exchange is made. The error must identify the conflicting option and variable names, not their values.
- **FR5:** When neither `--access-token-file` nor `APOLLO_ACCESS_TOKEN_FILE` is present, existing credential selection and authentication behavior must remain unchanged.
- **FR6:** Commands that need no authentication must not read the external token source or fail because it is unavailable or conflicts with another credential input.

### 2.2 Token format and use

- **FR7:** The source must contain one raw, non-empty UTF-8 bearer token. Rover must accept either no final line ending, one final LF, or one final CRLF. It must reject other whitespace, additional lines, an `Authorization` header or `Bearer` prefix, and characters that cannot occur in a bearer token. It must not parse or require JWT claims.
- **FR8:** Every attempt authenticated with an external token must send that token as `Authorization: Bearer`. It must not also send an API-key credential or substitute a profile credential.
- **FR9:** Rover must read the configured source anew before every Platform API attempt, including the first attempt, eligible retries, subsequent operations, and polling requests. This must hold for every Platform API request path.
- **FR10:** An atomic replacement completed before an attempt's source read must supply that attempt's token. A replacement concurrent with a read may supply the complete old token or complete new token, never a mixture. Concurrent requests must each obtain a complete token without changing another in-flight request's credential.
- **FR11:** After a source failure, Rover must not send the affected attempt, reuse an earlier token, use another credential, or initiate an interactive login. Its diagnostic must identify which of `--access-token-file` or `APOLLO_ACCESS_TOKEN_FILE` supplied the location, and the failure category, without revealing file contents.
- **FR12:** Rover must not attach the external token to requests other than Platform API requests, including introspection, plugin downloads, OAuth endpoints, and telemetry. It must not explicitly forward the token value to unrelated subprocesses.

### 2.3 Ownership and failures

- **FR13:** Rover must not create, modify, delete, or persist a copy of the external token source or its token. It must not acquire or use a refresh token for this credential, refresh it, or revoke it. The integrating application owns source security, atomic replacement, refresh, and cleanup.
- **FR14:** A body rejection must fail the affected operation without automatic replay. After a status rejection, Rover must read the source once more. If that read succeeds and yields a token different from the rejected one, Rover must resend the rejected request once with the new token. Rover must fail the operation if the token is unchanged, the read is a source failure, or the resent request is also rejected. Rover must not wait for the source to change, and must make at most one such resend per operation. When the operation fails, Rover must explain that the application-supplied credential was rejected and direct the caller to renew it in the supplying application, not to log Rover in.
- **FR15:** Apart from the single resend in FR14, external authentication must not add retries or change which transient failures are eligible for the command's existing retries. Each eligible retry must reread the source. A source failure must not itself trigger an automatic retry. The resend counts as an attempt of the same operation wherever Rover counts attempts, such as the retry count in its request trace headers.
- **FR16:** An authenticated permission denial must retain permission-denied behavior. It must not trigger token refresh, credential fallback, or an authentication retry.
- **FR17:** This feature must not introduce a new command recovery policy. Where a long-running command already permits later independent operations after an error, each later operation must read the source anew and may use a replacement supplied after the error.

### 2.4 Identity, privacy, and profile management

- **FR18:** `rover auth whoami` and `rover config whoami` must report the external token source as the active credential origin, naming which of `--access-token-file` or `APOLLO_ACCESS_TOKEN_FILE` supplied it, authenticate their Platform API identity lookup with the current external token, and work without a stored profile credential. They must not infer or claim the OAuth grant type that produced an external token, or require an OAuth userinfo request.
- **FR19:** Identity output must redact the token by default. Existing deliberate credential-unmasking options may expose it only when explicitly requested. Logs and errors, including debug output, must not reveal token contents. Telemetry must contain neither token contents nor the token-source location, including the value given to `--access-token-file` among a command's recorded arguments.
- **FR20:** `rover auth login` and `rover auth logout` must retain their selected-profile behavior independently of the external source. They must not read the external source, reject conflicting external-authentication inputs, change the source, or revoke its token merely because `--access-token-file` or `APOLLO_ACCESS_TOKEN_FILE` is present.
- **FR21:** Identity output and errors must use Rover's existing text and JSON output conventions. JSON output must retain the standard `json_version`, `data`, and `error` envelope.

### 2.5 Token file checks

- **FR22:** On every read, Rover must refuse the source if its configured location is itself a symbolic link. Symbolic links among the directories above it must be accepted.
- **FR23:** Where the platform has Unix file permissions, Rover must refuse the source if it is not owned by the user running Rover, if users other than its owner can read or write it, or if users other than its owner can write to the directory that holds it.
- **FR24:** Rover must refuse a source larger than 16 KiB, and must not read more than that from it.
- **FR25:** On Windows, Rover must apply FR22 and FR24 but not FR23.
- **FR26:** A refused source is a source failure under FR11. Its diagnostic must name the check that failed and how to fix it, without revealing the file's contents.

## 3. Acceptance criteria

### No separate Rover login

**Given** a valid external token source and no stored Rover credentials, **when** an authenticated Platform API command runs, **then** its requests use the external bearer token without a login prompt or credential-store write.

### Existing authentication remains available

**Given** neither `--access-token-file` nor `APOLLO_ACCESS_TOKEN_FILE`, **when** commands use an API key, client credentials, or a stored OAuth login, **then** credential selection and behavior remain unchanged.

### Conflicting runtime credentials

**Given** an external source, supplied by either the option or the variable, and any of `APOLLO_KEY`, `APOLLO_CLIENT_ID`, or `APOLLO_CLIENT_SECRET`, including a partial client-credentials pair, **when** an authenticated operation starts, **then** it fails with the conflicting option and variable names, without a token exchange, Platform API request, or disclosure of their values.

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

**Given** an empty file, invalid UTF-8, whitespace inside a token, extra lines, a prefixed header value, or an invalid bearer-token character, **when** Rover needs the token, **then** the affected attempt is not sent and the diagnostic does not repeat the contents.

### Rotation without restart

**Given** a running command that has used token A, **when** the application atomically replaces the source with token B before the next attempt's read, **then** that attempt uses token B without restarting Rover.

### Rotation during an eligible retry

**Given** a transient failure eligible for an existing retry and token A on the failed attempt, **when** the source is replaced with token B before the retry reads it, **then** the retry uses token B without adding an authentication-specific attempt.

### Concurrent requests and replacement

**Given** concurrent Platform API attempts and an atomic token replacement, **when** source reads overlap the replacement, **then** every request uses a complete token A or complete token B, and replacing the source does not mutate headers on a request already in flight.

### Source failure after a successful request

**Given** an attempt that succeeded with token A, **when** the source is removed, becomes unreadable, becomes invalid, or fails a token file check before the next source read, **then** the next attempt is not sent and Rover does not reuse token A or a saved credential.

### Source repair in a continuing command

**Given** a source failure in a command whose existing policy permits later independent operations, **when** the application repairs the source before the next independent operation, **then** that operation can authenticate with the repaired source without restarting Rover.

### Rejected credentials and permissions

**Given** an attempt with token A that receives a status rejection, **when** the source holds token B by the time Rover rereads it, **then** Rover resends the rejected request once with token B, and the command continues without a restart.

**Given** an attempt with token A that receives a status rejection, **when** the source still holds token A, **then** the operation fails at once with the rejection, without a resend and without waiting for the source to change.

**Given** an attempt with token A that receives a status rejection and a resend with token B that also receives a status rejection, **when** Rover handles the second rejection, **then** the operation fails without a further attempt.

**Given** an attempt that receives a status rejection, **when** rereading the source is a source failure, **then** the operation fails without a resend.

**Given** a body rejection, **when** a new token subsequently appears, **then** Rover reports the rejection without automatically replaying the rejected operation.

**Given** an authenticated identity without permission, **when** the Platform API denies the operation, **then** Rover reports a permission failure without changing credentials or initiating refresh.

### Token file checks

**Given** a source location that is a symbolic link to an otherwise valid token file, **when** Rover needs the token, **then** the attempt is not sent and the diagnostic says the location is a symbolic link.

**Given** a valid token file in a directory reached through a symbolic link, **when** Rover needs the token, **then** it uses the token.

**Given** a platform with Unix file permissions and a token file that other users can read, that another user owns, or whose directory other users can write to, **when** Rover needs the token, **then** the attempt is not sent and the diagnostic names the failed check and how to fix it.

**Given** a token file larger than 16 KiB, **when** Rover needs the token, **then** the attempt is not sent and Rover reads no more than 16 KiB of it.

**Given** Windows and a token file that would fail only the Unix permission checks, **when** Rover needs the token, **then** it uses the token.

### Identity reporting

**Given** an external source and no stored credential, **when** either identity command runs, **then** it uses the current token for a Platform API identity lookup, reports the external token source as the origin, naming the option or variable that supplied it, makes no userinfo request, and does not claim a known grant type. Text and JSON redact the token unless the caller explicitly requests unmasking; JSON retains the standard envelope.

### Destination isolation and diagnostics

**Given** external authentication, **when** Rover makes Platform API requests and requests to unrelated destinations, **then** only Platform API requests carry the external token.

**Given** successful and failing source reads, malformed contents, and rejected tokens, **when** output, debug logs, and telemetry are inspected, **then** none exposes token contents except explicitly requested identity unmasking, and telemetry does not include the token-source location, including the value of `--access-token-file`.

### Local and profile-management commands

**Given** an unavailable external source and conflicting runtime credential variables, **when** a command that needs no authentication runs, **then** external authentication does not cause it to fail or read the source.

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
