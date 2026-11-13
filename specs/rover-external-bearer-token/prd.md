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
4. Prevent accidental identity changes, credential fallback, or token disclosure.
5. Preserve existing authentication behavior when the integration is not enabled.

## 4. Proposed integration

### One input for both command lifetimes

The application supplies `APOLLO_ACCESS_TOKEN_FILE` in Rover's child-process environment. Its value identifies an application-managed file containing the current raw Apollo OAuth access token, with an optional final newline.

The application creates the file before invoking Rover, restricts access to the file and its containing directory, and replaces it atomically when a fresh token is available. Rover only reads it. Rover does not create, modify, delete, or copy it into a profile or credential store.

Rover rereads the configured location before every Platform API HTTP attempt, including retries and later polling requests. It does not retain a previous token as a fallback. A complete token replacement visible before a read must be used by that attempt; a replacement concurrent with a read may yield the complete old or complete new token.

This single input covers short-lived and long-running commands. A second input carrying the raw token directly in an environment variable is unnecessary for the initial scope. No token-value command-line flag, helper executable, or local IPC protocol is proposed.

### Credential selection

An external token replaces the credential normally obtained from the selected profile. Profile settings still apply; supplying a token does not change endpoint selection or unrelated configuration.

An external token combined with `APOLLO_KEY`, `APOLLO_CLIENT_ID`, or `APOLLO_CLIENT_SECRET` is a configuration error when authentication is needed. Rover must not guess which runtime identity the caller intended or exchange client credentials unnecessarily.

The new input is process-environment-only. It cannot be supplied through profile settings or project configuration. Commands that do not need authentication must not read the token file just because the variable is present.

### Lifecycle and errors

The desktop application refreshes the access token before it expires and publishes the replacement. Rover never receives or stores a refresh token and never attempts an OAuth refresh for the external credential.

An absent, unreadable, empty, or invalid token file fails the affected operation before sending an unauthenticated request. Rover must not reuse a cached token, fall back to a profile, or launch a login flow.

A credential rejection from the Platform API fails the affected operation without an automatic authentication retry. A new token appearing after that rejection does not authorize replaying the operation, particularly a mutation. Existing retry behavior for transient failures remains in place, with a fresh token read for each eligible attempt.

If an existing long-running command normally continues after an operation fails, it may continue under that same policy. A later independently scheduled operation can read a repaired or refreshed token file.

### Identity and logout

`rover auth whoami` and `rover config whoami` identify the external credential source without claiming that Rover performed an OAuth grant. They use the active token for the Platform API identity lookup and do not require a stored profile credential.

Existing explicit credential-unmasking behavior remains opt-in. Default identity output, logs, errors, and telemetry must not expose the token.

`rover auth login` and `rover auth logout` retain their selected-profile behavior. Neither manages the external token or its file, and neither reads that file just to log a profile in or out.

## 5. Security and compatibility boundaries

- The supplied value must be an access token accepted by the configured Platform API, with the required audience, permissions, and lifetime. Rover does not broaden its scopes or exchange it for another credential.
- Acceptance of tokens issued to the desktop application's OAuth client must be verified with the Platform/Identity owners before rollout. Existing bearer-header support does not establish that every client's token is accepted.
- Tokens are sent only to the configured Platform API endpoint. Existing endpoint overrides remain explicit trust decisions; they also control where this bearer token is sent.
- The feature does not attach the token to introspection, plugin-download, OAuth, or telemetry requests, or forward the token value to unrelated subprocesses.
- The application is responsible for secure file placement, permissions, atomic publication, and cleanup. Rover does not claim to make an insecurely shared file safe.
- Token contents and the token-file location must not enter telemetry. Invalid file content must not be echoed in diagnostics, including debug logs.
- Existing authentication works unchanged when `APOLLO_ACCESS_TOKEN_FILE` is absent.

## 6. Non-goals

- Logging the desktop application in, refreshing its tokens, or revoking its grant.
- Importing external tokens into Rover profiles or sharing a refresh token.
- Accepting Studio browser-session tokens or third-party identity-provider tokens.
- A generic credential-helper protocol or an IPC service.
- Automatic replay after an authentication rejection.
- Changing command permissions, selected endpoints, or the Platform API's authorization model.

## 7. Success criteria

- A desktop application can run an authenticated Rover command with no stored Rover credential.
- A running command uses a replacement access token on its next outgoing attempt without restarting.
- Both Platform API client paths satisfy the same token-selection and refresh behavior.
- Source failures never produce an unauthenticated request or an identity fallback.
- Existing API-key, client-credentials, and stored-login flows retain their behavior when the feature is disabled.
- Tests cover rotation, concurrent requests, retries, rejection, file failures, identity output, and secret redaction.
