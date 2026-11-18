# PRD: Application-supplied Platform API bearer tokens

## 1. Problem statement

A desktop application invokes Rover as a subprocess and already holds an Apollo OAuth access token. Today the application and Rover both require separate authentication. The application should be able to supply its token to Rover without creating a second login or giving Rover ownership of its OAuth grant.

The integration must support both short-lived commands and long-running commands that outlive an access token. Passing a token only through the environment at process startup cannot cover the latter: a parent cannot update the environment of an already-running child.

## 2. Current behavior

- Rover supports API keys, stored OAuth logins, and an OAuth client-credentials exchange.
- Credential resolution currently prefers `APOLLO_KEY`, then a client-credentials exchange, then the selected profile's credential.
- Both existing Platform API client paths can send OAuth access tokens using `Authorization: Bearer`.
- `APOLLO_KEY` is not a bearer-token input. Its value is sent as `x-api-key`.
- Stored OAuth credentials belong to Rover's login and logout lifecycle. An application-supplied token must not be treated as one of those stored logins.

## 3. Goals

1. Run authenticated Platform API commands without a Rover login or saved credential.
2. Allow a running Rover process to consume refreshed access tokens supplied by its parent application.
3. Keep login, refresh, revocation, and token-file ownership with the application.
4. Prevent accidental credential fallback or token disclosure. The token's identity is whatever the supplying application says it is: Rover does not pin it, so a replacement token for another account is accepted.
5. Preserve existing authentication behavior when the integration is not enabled.

## 4. Proposed integration

### One input for both command lifetimes

The application supplies `APOLLO_ACCESS_TOKEN_FILE` in Rover's child-process environment, or passes the same location with Rover's global `--access-token-file` option. Its value identifies an application-managed file containing the current raw Apollo OAuth access token, with an optional final newline.

The application creates the file before invoking Rover, restricts access to the file and its containing directory, and replaces it atomically when a fresh token is available. Rover only reads it. Rover does not create, modify, delete, or copy it into a profile or credential store. Rover also checks the file on every read and refuses one that is plainly unsafe to trust (see "Token file checks" below).

Rover rereads the configured location before every Platform API HTTP attempt, including retries and later polling requests. It does not retain a previous token as a fallback. A complete token replacement visible before a read must be used by that attempt; a replacement concurrent with a read may yield the complete old or complete new token.

This single input covers short-lived and long-running commands. A second input carrying the raw token directly in an environment variable is unnecessary for the initial scope. No token-value command-line flag, helper executable, or local IPC protocol is proposed.

### Token file checks

The token is a credential, so Rover checks the file the way SSH checks a private key, as a second line of defense behind the application's own handling. On every read, Rover refuses the file if:

- The configured path is itself a symlink. Symlinks in the folders above the file are allowed, since some systems' standard temporary locations use them.
- Other users can read or write the file, or the file is owned by someone other than the user running Rover.
- The folder that holds the file is owned by someone other than the user running Rover or the system administrator, or can be written to by its group or other users, which would let them replace the file. As with SSH's `StrictModes`, the folder's sticky bit does not excuse a world-writable folder, so a file placed directly in `/tmp` is refused.
- It is larger than any real access token could be. Rover reads at most a small fixed size and does not load an oversized file into memory.

On Windows, file and folder access is controlled by access-control lists rather than Unix permission bits, and checking those is out of scope for the first version. There, Rover applies only the symlink and size checks.

A refused file is handled like an unreadable one. The error says which check failed and how to fix it, but does not show the token or the file's contents.

### Credential selection

An external token replaces the credential normally obtained from the selected profile. Profile settings still apply; supplying a token does not change endpoint selection or unrelated configuration.

An external token combined with `APOLLO_KEY`, `APOLLO_CLIENT_ID`, or `APOLLO_CLIENT_SECRET` is a configuration error when authentication is needed, and Rover warns about it at startup. An empty `APOLLO_KEY` counts as absent. The exception is `rover dev`, where `APOLLO_KEY` is for the router Rover starts: Rover uses the external token for its own Platform API requests and passes `APOLLO_KEY` to the router as an environment variable only. Rover must not guess which runtime identity the caller intended or exchange client credentials unnecessarily.

The location comes only from the `--access-token-file` option or the `APOLLO_ACCESS_TOKEN_FILE` environment variable, and the option takes precedence when both are present, as it does for Rover's other global options. It cannot be supplied through profile settings or project configuration. Commands that do not need authentication must not read the token file just because the option or variable is present.

### Lifecycle and errors

The desktop application refreshes the access token before it expires and publishes the replacement. Rover never receives or stores a refresh token and never attempts an OAuth refresh for the external credential.

An absent, unreadable, empty, invalid, or refused token file fails the affected operation before sending an unauthenticated request. Rover must not reuse a cached token, fall back to a profile, or launch a login flow.

A credential rejection from the Platform API normally fails the affected operation. There is one exception, for the case where the token went stale between Rover reading it and the server checking it (for example, a token that expired while the machine was asleep, before the application could refresh it). When the Platform API rejects the credential with an HTTP 401 or 406 status, the statuses Rover already treats as credential rejections (an HTTP 403 is a permission denial, not a rejection), Rover rereads the token file once. If the file now holds a different token, Rover retries that request once with it. If the token is unchanged, the file can't be read or fails its checks, or the retry is rejected too, the operation fails. Rover doesn't wait for a new token to appear, and doesn't retry after a rejection reported any other way, such as in the body of a successful response, because part of the operation may already have run.

Existing retry behavior for transient failures remains in place, with a fresh token read for each eligible attempt.

If an existing long-running command normally continues after an operation fails, it may continue under that same policy. A later independently scheduled operation can read a repaired or refreshed token file.

### Identity and logout

`rover auth whoami` and `rover config whoami` identify the external credential source without claiming that Rover performed an OAuth grant. They use the active token for the Platform API identity lookup and do not require a stored profile credential.

Existing explicit credential-unmasking behavior remains opt-in. Default identity output, logs, errors, and telemetry must not expose the token.

`rover auth login` and `rover auth logout` retain their selected-profile behavior. Neither manages the external token or its file, and neither reads that file just to log a profile in or out.

## 5. Security and compatibility boundaries

- The supplied value must be an access token accepted by the configured Platform API, with the required audience, permissions, and lifetime. Rover does not broaden its scopes or exchange it for another credential.
- Acceptance of tokens issued to the desktop application's OAuth client must be verified with the Platform/Identity owners before rollout. Existing bearer-header support does not establish that every client's token is accepted.
- The single retry after a rejection relies on the Platform API rejecting an expired or invalid access token with an HTTP 401 before it runs any part of the operation. Staging returns 401 for a well-formed but unverifiable token from its gateway authentication layer. Before rollout, Platform/Identity owners must confirm the same holds for expired tokens and for every operation Rover sends.
- Tokens are sent only to the configured Platform API endpoint. Existing endpoint overrides remain explicit trust decisions; they also control where this bearer token is sent.
- The feature does not attach the token to introspection, plugin-download, OAuth, or telemetry requests, or forward the token value to unrelated subprocesses.
- The application is responsible for secure file placement, permissions, atomic publication, and cleanup. Rover refuses files that fail its token file checks, but those checks don't make an insecurely managed file safe.
- Token contents and the token-file location, including the value of `--access-token-file` among a command's recorded arguments, must not enter telemetry. Invalid file content must not be echoed in diagnostics, including debug logs.
- Existing authentication works unchanged when neither `--access-token-file` nor `APOLLO_ACCESS_TOKEN_FILE` is present.

## 6. Non-goals

- Logging the desktop application in, refreshing its tokens, or revoking its grant.
- Importing external tokens into Rover profiles or sharing a refresh token.
- Accepting Studio browser-session tokens or third-party identity-provider tokens.
- A generic credential-helper protocol or an IPC service.
- Replaying an operation after an authentication rejection, beyond the single retry with a changed token described above.
- Changing command permissions, selected endpoints, or the Platform API's authorization model.

## 7. Success criteria

- A desktop application can run an authenticated Rover command with no stored Rover credential.
- A running command uses a replacement access token on its next outgoing attempt without restarting.
- Both Platform API client paths satisfy the same token-selection and refresh behavior.
- Source failures never produce an unauthenticated request or an identity fallback.
- Existing API-key, client-credentials, and stored-login flows retain their behavior when the feature is disabled.
- A long-running command whose token expired before the application replaced it recovers on its next request, without a restart and without running any operation twice.
- Tests cover rotation, concurrent requests, retries, rejection, file failures, each token file check, identity output, and secret redaction.
