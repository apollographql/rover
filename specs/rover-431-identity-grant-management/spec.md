# Spec: Rover identity and OAuth grant management

- **PRD:** [prd.md](./prd.md)

## 1. Purpose and scope

This spec defines the observable contract for managing client-credential pairs through `rover api-key` (PRD Part A) and OAuth grants through `rover auth` (PRD Part B). It covers every requirement in the PRD: A1.1–A1.6, B1.1, B2.1–B2.4, B3.1–B3.7, and the cross-cutting requirements of PRD §8.

It describes what Rover must do under which conditions, and the exact user-facing surface involved: verbs, arguments, flags, message text, JSON fields, and error classes. It does not describe how any of it is built; implementation approach belongs in a separate implementation plan, one per slice.

Two things are deliberately outside it. **Credential resolution** keeps the chain it has today — `APOLLO_KEY`, then an OAuth client-credentials exchange from `APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET`, then the selected profile's stored credential — and nothing here changes it. **Authorization** is the Platform API's: Rover never decides whether a caller may perform an action, it only reports the server's decision in a consistent way (§3.10).

Parts of this contract depend on Platform API capability that does not exist yet (PRD §9). The contract is written against the full capability; §3.11 states what a Rover release must do while a piece of it is missing.

Requirements are tagged with the PRD requirement they realize, e.g. `(A1.3)`.

## 2. Terminology

- **Client-credential pair** (or **pair**): an OAuth 2.0 `client_id`/`client_secret` registered under an organization for the client-credentials grant. Identified by its **client ID**. Distinct from an organization API key, even though both are managed through `rover api-key`.
- **API key**: an organization API key of type `operator` or `subgraph`, as `rover api-key` manages today. Identified by its **key ID**.
- **Secret**: a pair's `client_secret`. A pair may hold more than one secret at once during a rotation's grace period.
- **Grant**: the OAuth 2.0 authorization issued to a user or a pair as the result of one authorization-code, device-code, or client-credentials exchange, together with every token derived from it. Identified by its **grant ID**. Never called a "session" in any output (PRD §2.3).
- **Grant type**: how a grant was established. Exactly one of three: `authorization_code` (a browser login), `device_code` (a `--no-browser` login), `client_credentials` (a pair's token exchange).
- **Principal**: whoever a grant belongs to — a user, or a pair.
- **Current grant**: the grant whose tokens the current invocation is authenticating with. An invocation authenticating with an API key has no current grant.
- **Rover's OAuth client**: the OAuth client Rover's own `rover auth login` flows authenticate against — the effective value of `APOLLO_OAUTH_CLIENT_ID` for the invocation.
- **Self-service scope**: acting on the caller's own grants. The default for every `rover auth grants` verb.
- **Organization scope**: acting on every grant in one organization. Selected only by `--org <ORGANIZATION_ID>`.
- **Per-user sweep**: `rover auth grants revoke --org <ORGANIZATION_ID> --user <USER_ID> --all` (§3.8).

---

## 3. Functional requirements

### 3.1 The `client-credentials` key type (A1.1)

- **FR1**: `rover api-key create` must accept `client-credentials` as a third value of its `TYPE` argument, alongside `operator` and `subgraph`:

  ```
  rover api-key create <ORGANIZATION_ID> client-credentials <NAME> --graph-id <GRAPH_ID>... [--secret-lifetime-days <DAYS>]
  ```

- **FR2**: `--graph-id` is required for `client-credentials` and repeatable. Omitting it is a usage error, rejected before any request is made. A graph ID given more than once is sent once. Rover does not enforce the upper bound on how many graphs a pair may name, and does not check that each graph exists or belongs to the organization; it surfaces whatever validation error the Platform API returns.
- **FR3**: `--secret-lifetime-days` is optional and takes a whole number of days. A value that is not a non-negative whole number is a usage error. Rover does not enforce a range; an out-of-range value is sent and the Platform API's validation error is surfaced (PRD A1.1).
- **FR4**: `--graph-id` and `--secret-lifetime-days` are accepted only with `client-credentials`, and `--subgraph-config` is not accepted with it. Any other combination is a usage error. Combinations valid today for `operator` and `subgraph` are unchanged.
- **FR5**: Rover must request exactly the `rover:cli` scope for every pair it creates and must expose no flag that selects a scope or a role.
- **FR6**: On success, Rover must print the client ID and the secret to stdout, whether or not stdout is a terminal, so that CI setup can capture them. It must also print the pair's name, its graphs, and when the secret expires.
- **FR7**: Rover must print this line to stderr after a successful create in text mode:

  > Save this secret now. Rover can't show it again.

- **FR8**: The JSON payload for create must be:

  ```json
  {
    "json_version": "1",
    "data": {
      "key_type": "ClientCredentials",
      "id": "c_8f2a…",
      "client_id": "c_8f2a…",
      "client_secret": "s_…",
      "secret_expires_at": "2028-09-25T16:00:00Z",
      "name": "ci-deploy",
      "graphs": ["inventory", "checkout"],
      "scopes": ["rover:cli"],
      "success": true
    },
    "error": null
  }
  ```

  `id` and `client_id` carry the same value; `id` exists so a consumer that already reads `id` from `rover api-key create` output works for every key type. `graphs` is in the order the Platform API reports them.
- **FR9**: The JSON payload for creating an `operator` or `subgraph` key is unchanged.

### 3.2 Listing (A1.2)

- **FR10**: `rover api-key list <ORGANIZATION_ID>` must report the organization's pairs in addition to its API keys. It must page through every page of both sets before printing anything, so the output is always the complete set.
- **FR11**: `rover api-key list` must accept `--type <TYPE>`, where `<TYPE>` is `operator`, `subgraph`, or `client-credentials`, repeatable. When given, only keys and pairs of the named types are reported. When omitted, every type is reported.
- **FR12**: For each pair, the list must report at least: name, client ID, graphs, creation time, and who created it. Once the Platform API reports a last-used time for pairs, the list must report it too (§3.11).
- **FR13**: No secret, and no part of a secret, may appear in list output in any format.
- **FR14**: The text output for API keys must be unchanged when the organization has no pairs, or when pairs are filtered out by `--type`. When pairs are reported, they appear in a separate table after the API key table, titled `Client-credential pairs`.
- **FR15**: The JSON payload must keep the existing `keys` array with every field it carries today, add a `key_type` field to each entry in it, and add a sibling `client_credentials` array:

  ```json
  {
    "json_version": "1",
    "data": {
      "keys": [
        {
          "id": "key-123",
          "name": "router-prod",
          "key_type": "Operator",
          "created_at": "2026-01-04T12:00:00Z",
          "expires_at": null
        }
      ],
      "client_credentials": [
        {
          "key_type": "ClientCredentials",
          "id": "c_8f2a…",
          "client_id": "c_8f2a…",
          "name": "ci-deploy",
          "graphs": ["inventory", "checkout"],
          "scopes": ["rover:cli"],
          "created_at": "2026-09-25T16:00:00Z",
          "created_by": { "id": "user-123", "name": "Grace Hopper" }
        }
      ],
      "success": true
    },
    "error": null
  }
  ```

  `created_by.name` is `null` when the Platform API reports no display name. Once last-used time is available, each pair entry gains `last_used_at`, which is `null` only for a pair that has never obtained a token.
- **FR16**: If the caller cannot list the organization's pairs — because they lack the permission, or because the organization is not enrolled in client-credential support — and `--type` was not given, Rover must report the API keys exactly as it does today and set `client_credentials` to `null`. It must print nothing about the pairs it could not list. `null` means "not available to this caller"; `[]` means "there are none."
- **FR17**: If `--type client-credentials` was given and the caller cannot list pairs, the command must fail with the permission error of §3.10.

### 3.3 Addressing a pair by ID (A1.3, A1.4, A1.5)

- **FR18**: `rover api-key delete`, `rover api-key rename`, and `rover api-key rotate` take a single positional `<ID>` after `<ORGANIZATION_ID>`, as `delete` and `rename` do today. No flag distinguishes a client ID from a key ID.
- **FR19**: If `<ID>` names a pair in the organization, the command acts on that pair. Otherwise it acts on `<ID>` as a key ID, exactly as the command does today.
- **FR20**: If Rover cannot determine whether `<ID>` names a pair — because the caller cannot list the organization's pairs — it must act on `<ID>` as a key ID exactly as today, with no additional output. A caller who manages only API keys must observe no change in `delete` or `rename`.

### 3.4 Rotating a secret (A1.3)

- **FR21**: `rover api-key rotate <ORGANIZATION_ID> <CLIENT_ID> [--grace-period-days <DAYS>]` must mint a new secret for the pair and schedule every other secret the pair holds to stop working after the grace period.
- **FR22**: With no `--grace-period-days`, the grace period is zero: every previous secret stops working immediately. `--grace-period-days 0` means the same. Rover must not substitute a default of its own.
- **FR23**: `--grace-period-days` takes a whole number of days. A value that is not a non-negative whole number is a usage error. Rover does not enforce the upper bound; an out-of-range value is sent and the Platform API's validation error is surfaced.
- **FR24**: On success, Rover must print the new secret to stdout, with its expiry, under the same once-only rule as create (FR6, FR7). The pair's name and graphs are unchanged by rotation.
- **FR25**: Rover must always state when the previous secrets stop working. A hard cutover is never silent.

  Required text, zero grace period:
  > Every previous secret for `ci-deploy` has stopped working. To keep the old secret working while you roll out a new one, pass `--grace-period-days <DAYS>`.

  Required text, non-zero grace period:
  > Every previous secret for `ci-deploy` keeps working until 2026-09-26T16:00:00Z.

- **FR26**: The JSON payload for rotate must be:

  ```json
  {
    "json_version": "1",
    "data": {
      "key_type": "ClientCredentials",
      "id": "c_8f2a…",
      "client_id": "c_8f2a…",
      "client_secret": "s_…",
      "secret_expires_at": "2028-09-25T16:00:00Z",
      "grace_period_days": 1,
      "previous_secrets_expire_at": "2026-09-26T16:00:00Z",
      "success": true
    },
    "error": null
  }
  ```

  With a zero grace period, `previous_secrets_expire_at` is the time of rotation, not `null`.
- **FR27**: `rotate` on an ID that is not a pair (FR19) must fail without making any change, with its own error code (§3.10).

  Required text:
  > `key-123` isn't a client-credential pair in organization `acme`. `rover api-key rotate` supports client-credential pairs only.

### 3.5 Deleting and renaming a pair (A1.4, A1.5)

- **FR28**: `rover api-key delete <ORGANIZATION_ID> <CLIENT_ID>` must delete the pair. Like deleting an API key today, it does not prompt.
- **FR29**: On success, Rover must state that the pair can no longer obtain tokens and that tokens already issued outlive it.

  Required text:
  > Deleted client-credential pair `ci-deploy` (`c_8f2a…`). It can no longer obtain tokens. Access tokens it already holds keep working until they expire, for up to 15 minutes.

- **FR30**: The JSON payload for deleting a pair keeps the existing `id` field and adds `key_type` (`"ClientCredentials"`). The JSON payload for deleting an API key keeps every existing field and adds `key_type`.
- **FR31**: `rover api-key rename` on a pair must fail without making any change, with its own error code (§3.10), until the Platform API supports renaming a pair.

  Required text:
  > `c_8f2a…` is a client-credential pair. Client-credential pairs can't be renamed.

### 3.6 The current grant's type (B1.1)

- **FR32**: `rover auth whoami` must report the grant type of the current grant, in addition to everything it reports today. Every existing field, including `origin`, keeps its current value.
- **FR33**: The JSON payload gains one field, `grant_type`, whose value is one of:
  - `"authorization_code"`, `"device_code"`, or `"client_credentials"`, per §2;
  - `"unknown"`, for an OAuth login stored by a version of Rover that did not record its grant type;
  - `null`, when the invocation authenticated with an API key and so has no grant.
- **FR34**: Text output gains a `Grant Type` row, labeled `Browser login`, `Device code (--no-browser)`, `Client credentials`, or `Unknown — log in again to record it`. The row is omitted when `grant_type` is `null`.
- **FR35**: `rover auth login` must record which grant type it established, in both its browser and `--no-browser` forms.
- **FR36**: A credential stored by this version of Rover must remain usable by earlier versions of Rover that support OAuth login. Recording the grant type must not make a shared configuration directory unreadable to the older version.

### 3.7 Listing grants (B2.1, B2.2, B2.4, B3.1, B3.7)

- **FR37**: `rover auth grants list` must report every active grant belonging to the caller: grants of type `authorization_code` and `device_code`. Grants of type `client_credentials` belong to a pair, not a person, and never appear in the self-service list.
- **FR38**: Each grant must be reported with at least: its grant ID, its grant type, when it was created, when it was last used or refreshed, and whether it is the current grant. At most one grant is ever the current grant.
- **FR39**: A grant ID must always be printed in full, never truncated, so it can be passed back to `rover auth grants revoke`.
- **FR40**: Text output must label grant types as FR34 does, so a `--no-browser` grant is visibly distinct from a browser grant.
- **FR41**: Self-service scope requires a credential that belongs to a user. With any other credential, the command must fail before making a grants request.

  Required text:
  > `rover auth grants` without `--org` acts on your own grants, which requires logging in as a user. The current credential (`APOLLO_KEY` in the environment) doesn't belong to a user. Run `rover auth login`, or pass `--org <ORGANIZATION_ID>` to act on an organization's grants.

  The parenthetical names the credential's origin as `rover auth whoami` reports it.
- **FR42**: Self-service scope requires no organization role (B2.4).
- **FR43**: `rover auth grants list --org <ORGANIZATION_ID>` must report every active grant in the organization, across every member and every pair, with every field of FR38 plus the principal each grant belongs to.
- **FR44**: A caller without the Platform API's permission for organization-scoped grant access must get the permission error of §3.10, never an empty list.
- **FR45**: With no active grants, text output must say so and exit 0.

  Required text, self-service:
  > You have no active grants.

  Required text, organization scope:
  > Organization `acme` has no active grants.

- **FR46**: The JSON payload must be:

  ```json
  {
    "json_version": "1",
    "data": {
      "scope": "organization",
      "organization_id": "acme",
      "grants": [
        {
          "grant_id": "g_4c1e…",
          "grant_type": "device_code",
          "created_at": "2026-09-20T09:12:00Z",
          "last_used_at": "2026-09-25T14:03:00Z",
          "current": false,
          "principal": { "type": "user", "id": "user-123", "name": "Grace Hopper" }
        }
      ],
      "success": true
    },
    "error": null
  }
  ```

  `scope` is `"self"` or `"organization"`. In self-service scope, `organization_id` is `null` and `principal` is omitted. `principal.type` is `"user"` or `"client"`. `grants` is `[]` when there are none.

### 3.8 Revoking grants (B2.3, B3.2–B3.6)

#### Argument rules

- **FR47**: `rover auth grants revoke` accepts exactly one of these forms. Every other combination is a usage error, rejected before any request is made:

  | Form | Scope |
  |---|---|
  | `rover auth grants revoke <GRANT_ID>` | one of the caller's own grants |
  | `rover auth grants revoke --org <ORGANIZATION_ID> <GRANT_ID>` | one grant in the organization |
  | `rover auth grants revoke --org <ORGANIZATION_ID> --user <USER_ID> --all [--confirm]` | every grant of one user under the organization's OAuth clients |

- **FR48**: In particular, each of these must be a usage error: `--all` without `--user`; `--user` without `--all`; `--user` or `--all` without `--org`; `<GRANT_ID>` together with `--user` or `--all`; and no `<GRANT_ID>` and no `--all`. No combination of arguments revokes every grant in an organization (B3.6).
- **FR49**: `--confirm` is accepted with every form. It has an effect only on the per-user sweep, where it skips the prompt.

#### Revoking one grant

- **FR50**: Revoking one grant revokes exactly that grant. Every other grant belonging to the same principal stays valid.
- **FR51**: Revoking one grant does not prompt.
- **FR52**: In self-service scope, a grant ID that does not name one of the caller's active grants must fail with the not-found error of §3.10, whether the grant does not exist, has already been revoked, or belongs to someone else. The message must not reveal which. In organization scope, the same holds for a grant ID that does not name an active grant in that organization.

  Required text, self-service:
  > Grant `g_4c1e…` isn't one of your active grants. Run `rover auth grants list` to see them.

  Required text, organization scope:
  > Grant `g_4c1e…` isn't an active grant in organization `acme`. Run `rover auth grants list --org acme` to see them.

- **FR53**: Revoking the current grant is allowed. Once the Platform API confirms the revocation, Rover must remove the current profile's stored OAuth credential, as `rover auth logout` does. If the revocation fails, the stored credential must be left in place.
- **FR54**: On success, Rover must confirm what it revoked.

  Required text:
  > Revoked grant `g_4c1e…` (Device code (--no-browser), created 2026-09-20T09:12:00Z).

  Required text, additionally, when it was the current grant:
  > That was the grant this profile was using. Profile `default` is now logged out.

#### The per-user sweep

- **FR55**: The per-user sweep must revoke the user's grants under Rover's OAuth client and under every pair registered in the organization.
- **FR56**: Before revoking anything, Rover must enumerate every pair in the organization, through every page. If enumeration fails, Rover must revoke nothing and fail.
- **FR57**: Unless `--confirm` was passed, Rover must then prompt on stderr, naming the user, the organization, and every OAuth client it will revoke under, and ask a yes/no question that defaults to no.

  Required text:
  > This revokes every grant `user-123` holds under these OAuth clients in organization `acme`:
  >   - Rover (personal browser and device-code logins, in every organization)
  >   - `ci-deploy` (`c_8f2a…`)
  >   - `nightly-checks` (`c_91be…`)
  > It doesn't revoke access tokens they already hold, and doesn't stop them logging in again.
  > Revoke? [y/N]

  Once the Platform API can enumerate grants (§3.11), the prompt must also list the user's active grants in the organization, as `rover auth grants list --org` would report them.
- **FR58**: Declining the prompt must revoke nothing, print the following to stderr, and exit 0.

  Required text:
  > Revocation cancelled. Nothing was revoked.

- **FR59**: When stdin is not a terminal and `--confirm` was not passed, Rover must not wait for an answer. It must revoke nothing and fail with its own error code (§3.10).

  Required text:
  > Revoking every grant for `user-123` needs confirmation, and there's no terminal to ask on. Pass `--confirm` to proceed without a prompt.

- **FR60**: If `<USER_ID>` does not name a current member of the organization, Rover must print a warning to stderr and proceed, including in the prompt. A departing user is often removed from the organization before their grants are swept, and the sweep must still work for them.

  Required text:
  > Warning: `user-123` isn't a current member of organization `acme`. Rover will still revoke any grants they hold under the clients below.

- **FR61**: Rover must attempt every client even after one fails. It must not stop at the first failure.
- **FR62**: The output must report the outcome for each client individually, by name and ID. A client under which the user had no grant is reported as `revoked`, not as a failure.
- **FR63**: If revocation failed under any client, the command must exit non-zero with its own error code (§3.10) and must name every client that still needs a retry. It must never report overall success while a revocation it attempted failed.

  Required text, success:
  > Revoked every grant `user-123` holds under 3 OAuth clients in organization `acme`.
  > Access tokens they already hold keep working until they expire. This doesn't stop `user-123` logging in again; remove them from the organization to do that.

  Required text, partial failure:
  > Revoked grants for `user-123` under 2 of 3 OAuth clients in organization `acme`. Revocation failed under:
  >   - `nightly-checks` (`c_91be…`): <the Platform API's error message>
  > Run the same command again to retry. Clients already revoked are unaffected by a retry.

- **FR64**: Running the same sweep again must be safe: revoking under a client where the user holds no grant is a success, so a retry after partial failure only changes the clients that failed.
- **FR65**: The per-user sweep does not revoke Studio web-login sessions, and does not reach pairs registered under any other organization.
- **FR66**: The target user may be the caller. Rover must not special-case it: the sweep proceeds, and the caller's stored credential is not removed by the sweep.
- **FR67**: The JSON payload for the per-user sweep must be:

  ```json
  {
    "json_version": "1",
    "data": {
      "organization_id": "acme",
      "user_id": "user-123",
      "user_is_member": true,
      "clients": [
        { "client_id": "rover", "name": "Rover", "kind": "rover", "outcome": "revoked", "error": null },
        { "client_id": "c_8f2a…", "name": "ci-deploy", "kind": "client_credentials", "outcome": "revoked", "error": null },
        { "client_id": "c_91be…", "name": "nightly-checks", "kind": "client_credentials", "outcome": "failed", "error": "<the Platform API's error message>" }
      ],
      "success": false
    },
    "error": {
      "message": "Revocation failed under 1 of 3 OAuth clients.",
      "code": "<partial-revocation code>"
    }
  }
  ```

  `outcome` is `"revoked"` or `"failed"`. `kind` is `"rover"` for Rover's OAuth client and `"client_credentials"` for a pair; `client_id` for Rover's OAuth client is the effective `APOLLO_OAUTH_CLIENT_ID`. `data` carries every client on both success and partial failure, so a runbook can read which clients to retry without parsing `error.message`. A declined prompt produces `"success": true`, `"clients": []`, and an additional `"cancelled": true`.
- **FR68**: The JSON payload for revoking one grant must report the revoked grant with the fields of FR46, plus `"logged_out_profile"`: the profile name when FR53 removed a stored credential, otherwise `null`.

### 3.9 Secrets and logging (PRD §8.2)

- **FR69**: A secret may be printed only in the output of the create or rotate call that minted it. It must never appear in debug or trace logging at any level, in telemetry, in an error message, or in the output of any other command.
- **FR70**: No command in this spec may print a full access token or refresh token, in any format, at any log level. The one exception is `rover auth whoami --insecure-unmask-key`, which keeps its existing opt-in behavior unchanged; by default `whoami` masks the token, as it does today.
- **FR71**: A failed create or rotate must not print a secret, even if the Platform API returned one before the failure was detected.

### 3.10 Errors and permissions (A1.6, B3.7, PRD §3, PRD §8.1)

- **FR72**: Rover must not evaluate roles, ownership, or feature-flag enrollment itself. Every permission decision is the Platform API's, and Rover's job is to report it.
- **FR73**: A request the Platform API refuses for lack of permission must fail with a single, stable permission-denied error code, distinct from the code for an authentication failure. The message must name the action and the organization.

  Required text, Part A:
  > You don't have permission to manage client-credential pairs in organization `acme`. This requires the organization admin role, and during the initial rollout the organization must be enrolled in client-credential support.

  Required text, organization-scoped grants:
  > You don't have permission to manage grants across organization `acme`. This requires the organization's grant-management permission.

- **FR74**: The following failure classes must each have their own stable error code, surfaced as `error.code` in JSON output and in the printed error:
  1. Permission denied (FR73).
  2. The ID is not a client-credential pair, for `rotate` (FR27).
  3. A client-credential pair cannot be renamed (FR31).
  4. The credential does not belong to a user, for self-service grant verbs (FR41).
  5. The grant was not found (FR52).
  6. Confirmation is required and no terminal is available (FR59).
  7. The per-user sweep failed under one or more clients (FR63).
- **FR75**: Every code in FR74 must be documented in the generated error reference, with the same page-per-code treatment every other Rover error code gets.
- **FR76**: Usage errors (FR2–FR4, FR11, FR23, FR47–FR48) are reported the same way Rover reports any invalid argument today, and are always detected before any request is made.
- **FR77**: Every verb in this spec must support `--format json` through Rover's standard `{"json_version", "data", "error"}` envelope. Every timestamp in `data` is an RFC 3339 string in UTC. Every field in `data` is a typed value, never prose that a consumer would have to parse.

### 3.11 Availability while Platform API capability is missing (PRD §9)

- **FR78**: A Rover release must not advertise, in `--help` or published documentation, any verb, argument, flag, or output field whose backing Platform API capability is unavailable to that release.
- **FR79**: Until the Platform API can enumerate a user's own grants, enumerate an organization's grants, and revoke a single grant, `rover auth grants list` must not exist, and `rover auth grants revoke` must accept only the per-user sweep form. Passing a `<GRANT_ID>` must be a usage error, not a runtime failure.
- **FR80**: Until the Platform API reports a pair's last-used time, the list output must not carry a last-used column or a `last_used_at` field for pairs. Its absence is not a `null`.
- **FR81**: FR32–FR36 (grant type in `whoami`) do not depend on new Platform API capability and must not wait for it.
- **FR82**: Until the Platform API can enumerate grants, FR57's prompt lists only the OAuth clients it will revoke under, not the user's grants.

### 3.12 Compatibility (PRD §4.7)

- **FR83**: `rover api-key create`, `list`, `delete`, and `rename` for `operator` and `subgraph` keys must behave exactly as today, apart from the additive `key_type` fields of FR15 and FR30. Their existing text output, and every existing JSON field, is unchanged.
- **FR84**: `rover auth login`, `rover auth logout`, and `rover auth whoami` must behave exactly as today, apart from FR32–FR35.
- **FR85**: `APOLLO_KEY`, `APOLLO_CLIENT_ID`, and `APOLLO_CLIENT_SECRET` keep their current meaning and precedence.
- **FR86**: A pair created by `rover api-key create` must be usable immediately by setting `APOLLO_CLIENT_ID` to its client ID and `APOLLO_CLIENT_SECRET` to its secret, with no other configuration.

### 3.13 Documentation (PRD §8.3)

- **FR87**: The `rover api-key` documentation must describe the `client-credentials` type, the `rotate` verb, the zero-default grace period and how to avoid a hard cutover, and a worked CI setup that ends with `APOLLO_CLIENT_ID` and `APOLLO_CLIENT_SECRET` set.
- **FR88**: The `rover auth` documentation must describe the `grants` verbs, the difference between self-service and organization scope, and exactly what the per-user sweep does and does not revoke (FR57, FR65).
- **FR89**: No behavior may be documented that the shipping release does not implement (FR78).

---

## 4. Acceptance criteria

**CI setup end to end**
Given an organization admin authenticated with an `operator` API key, when `rover api-key create acme client-credentials ci-deploy --graph-id inventory --format json` runs, then `data.client_id` and `data.client_secret` are present. When those two values are exported as `APOLLO_CLIENT_ID` and `APOLLO_CLIENT_SECRET`, with `APOLLO_KEY` unset and no profile, then `rover subgraph publish inventory@current ...` succeeds.

**`--graph-id` is required for a pair and only for a pair**
When `rover api-key create acme client-credentials ci-deploy` runs with no `--graph-id`, then it fails with a usage error and no request is made. When `rover api-key create acme operator router --graph-id inventory` runs, then it fails with a usage error and no request is made.

**The secret is shown once**
Given a pair created in text mode, then stdout carries the secret and stderr carries the FR7 line. When `rover api-key list acme` runs afterward, in either format, then no part of the secret appears.

**List keeps the API key output intact**
Given an organization with API keys and no pairs, when `rover api-key list acme` runs, then its text output is identical to today's. Given pairs as well, then the API key table is unchanged and a `Client-credential pairs` table follows it.

**A caller who can't see pairs sees today's list**
Given a caller who may list API keys but not pairs, when `rover api-key list acme --format json` runs, then `data.keys` matches today's output plus `key_type`, `data.client_credentials` is `null`, and nothing about pairs is printed. When `rover api-key list acme --type client-credentials` runs, then it fails with the permission-denied code.

**Filtering by type**
Given API keys of both types and two pairs, when `rover api-key list acme --type client-credentials --format json` runs, then `data.keys` is `[]` and `data.client_credentials` has two entries.

**Rotation is immediate by default, and says so**
Given a pair, when `rover api-key rotate acme c_8f2a…` runs, then a new secret is printed, the zero-grace FR25 text is printed, and `data.previous_secrets_expire_at` is the time of rotation. A job that then authenticates with the old secret fails with an authentication error.

**Rotation with a grace period**
Given a pair, when `rover api-key rotate acme c_8f2a… --grace-period-days 1` runs, then the non-zero FR25 text names a time one day later. A job using the old secret succeeds before that time and fails after it.

**Rotate refuses an API key**
Given an `operator` key `key-123`, when `rover api-key rotate acme key-123` runs, then it fails with the FR27 text and the not-a-pair code, and the key is unchanged.

**Delete finds the pair by its client ID**
Given a pair, when `rover api-key delete acme c_8f2a…` runs, then the pair is deleted without a prompt and the FR29 text is printed. Given an API key instead, then `rover api-key delete acme key-123` behaves exactly as today.

**Rename refuses a pair**
Given a pair, when `rover api-key rename acme c_8f2a… new-name` runs, then it fails with the FR31 text and the pair is unchanged.

**`whoami` distinguishes browser from device code**
Given a profile logged in with `rover auth login` and another logged in with `rover auth login --no-browser`, when `rover auth whoami --format json` runs against each, then `data.grant_type` is `"authorization_code"` and `"device_code"` respectively, and `data.origin` is what it is today. Given a profile logged in by an earlier Rover version, then `data.grant_type` is `"unknown"`. Given `APOLLO_KEY`, then `data.grant_type` is `null`.

**An older Rover still reads the new credential**
Given a profile logged in by this version of Rover, when an earlier OAuth-capable version of Rover runs `rover auth whoami` against it, then it succeeds.

**Two grants, told apart, revoked one at a time**
Given a user logged in from a browser on one machine and with `--no-browser` on another, when `rover auth grants list` runs on the first, then two grants are listed, labeled `Browser login` and `Device code (--no-browser)`, with the first marked current. When `rover auth grants revoke <device-code grant ID>` runs, then the second machine's next command fails authentication and the first machine's commands keep working.

**Revoking the current grant logs the profile out**
When `rover auth grants revoke <current grant ID>` runs, then both FR54 lines are printed, `data.logged_out_profile` is `"default"`, and the next command on that profile fails for lack of a credential.

**Someone else's grant is simply not found**
Given a grant ID belonging to another user, when `rover auth grants revoke <that ID>` runs without `--org`, then it fails with the self-service FR52 text and the not-found code, and the grant stays valid.

**Self-service needs a user**
Given only `APOLLO_KEY` set to an `operator` key, when `rover auth grants list` runs, then it fails with the FR41 text and makes no grants request.

**Organization scope is never an empty list by default**
Given a member without the organization-scoped permission, when `rover auth grants list --org acme` runs, then it fails with the permission-denied code rather than printing an empty list.

**No combination revokes the whole organization**
When `rover auth grants revoke --org acme --all` runs, then it fails with a usage error and nothing is revoked. The same holds for `--user user-123` without `--all`, and for `--user user-123 --all` without `--org`.

**Offboarding a departing user**
Given user `user-123` with a browser login and a grant under pair `ci-deploy`, and user `user-456` with grants of their own, when `rover auth grants revoke --org acme --user user-123 --all --confirm` runs, then every client is reported `revoked`, the command exits 0, both of `user-123`'s grants stop refreshing, and every grant of `user-456` stays valid.

**Offboarding someone already removed**
Given `user-123` has already been removed from `acme`, when the same command runs, then the FR60 warning is printed, `data.user_is_member` is `false`, and the sweep proceeds.

**Declining the prompt**
When `rover auth grants revoke --org acme --user user-123 --all` runs in a terminal and the answer is `n`, or the answer is empty, then nothing is revoked, the FR58 text is printed, and the exit code is 0.

**No terminal, no `--confirm`**
When the same command runs with stdin not a terminal and no `--confirm`, then it fails with the FR59 text and the confirmation-required code, and nothing is revoked.

**Partial failure is loud and retryable**
Given the Platform API fails revocation under `nightly-checks` only, when the per-user sweep runs with `--confirm`, then every other client is revoked, the partial-failure FR63 text names `nightly-checks`, the exit code is non-zero, and `data.clients` reports each client's outcome. When the command is run again and the API has recovered, then every client is reported `revoked` and it exits 0.

**Enumeration fails before anything is revoked**
Given listing the organization's pairs fails partway through, when the per-user sweep runs, then no revocation request is made and the command fails.

**No secret in logs**
Given `--log trace`, when `rover api-key create` and `rover api-key rotate` run, then the secret appears in their stdout and nowhere in stderr.

**Before grant enumeration ships**
Given a release in which the Platform API cannot enumerate grants, then `rover auth grants --help` lists only `revoke`, `rover auth grants revoke --help` documents only the per-user sweep, and `rover auth grants revoke g_4c1e…` is a usage error.

---

## 5. Non-goals

- Any form of revocation wider than one user (PRD §5). No verb, flag, or argument combination revokes every grant in an organization.
- Rotating `operator` or `subgraph` API keys (FR27).
- A scope or role selector for pairs (FR5).
- A confirmation prompt on `rover api-key delete` for any key type (FR28).
- Filtering the organization-scoped grant list by user, pair, or grant type. The list is complete by design; filtering is `jq`'s job.
- Labeling grants by host, device, or IP address. Grant type is the label this spec requires (FR40).
- Studio web-login sessions, which are a different resource with their own revocation.
- Cross-organization read of any kind, including for support engineers.
- The Platform API's own design: field names, permission names, and storage.
- Changing credential resolution, or storing a pair as a new kind of profile credential.

---

## 6. Decisions

Decisions taken while drafting, and their reasoning.

- **A pair is addressed by the same positional `<ID>` as an API key, and Rover works out which it is** (FR18–FR20), over a `--type` flag on `delete`/`rename`/`rotate` or a separate verb set. The PRD's command shapes carry no type, and a client ID is already unambiguous within an organization. The one hazard is a caller who cannot list pairs — an operator-key holder in an organization not enrolled in the rollout — for whom a "which is it?" lookup would turn a working `delete` into a permission error. FR20 closes that by falling through to today's behavior silently, which is also what makes FR83's no-regression promise hold.

- **`rover api-key delete` gets no prompt for pairs** (FR28), resolving the PRD's "existing delete semantics carry over." Deleting an API key does not prompt today. Adding a prompt for one key type only would make the same verb behave differently by the shape of the ID, and deleting a pair is exactly the leaked-credential response, where a prompt is friction at the worst moment. The per-user sweep is the one place this spec adds a prompt, because it fans out across clients the operator may not have in mind.

- **List output keeps `keys` and adds `client_credentials` beside it** (FR15), over merging pairs into `keys`. A pair has a different field set — two identifiers, graphs, no expiry of its own — and a script that iterates `data.keys[].id` today would start receiving client IDs it can pass to nothing but `delete`. A sibling array is additive; a merged one is a silent change of meaning. The PRD's "list merges two queries" is honored in the text output, where both appear in one command.

- **`client_credentials: null` means "not available to you," `[]` means "none"** (FR16). Without the distinction, an operator-key holder in an unenrolled organization and an admin in an organization with no pairs would see identical output, and the first would conclude there is nothing to clean up. The distinction costs nothing and keeps FR83 intact, because the error is not raised unless the caller asked for pairs specifically.

- **`key_type` is spelled `"ClientCredentials"`** (FR8), matching the `"Operator"`/`"Subgraph"` that `rover api-key create` already emits in JSON, over the CLI's `client-credentials` spelling. One field with two spelling conventions depending on the key type would be worse than one convention that differs from the CLI argument.

- **Range checks are the Platform API's, not Rover's** (FR2, FR3, FR23). The PRD already makes this call for `--secret-lifetime-days`; this spec extends it to `--graph-id`'s upper bound and `--grace-period-days`'s maximum so all three behave the same way. Rover rejects only what is syntactically not a number, because that is Rover's to parse. Hard-coding the API's current limits would make Rover wrong the day the API changes them.

- **Rotation always reports `previous_secrets_expire_at`, including for a zero grace period** (FR26), over `null` for an immediate cutover. The PRD's success metric is that a hard cutover is never silent; a field that is sometimes a timestamp and sometimes absent invites a script to treat absence as "nothing expired."

- **Grant type is recorded at login, and `"unknown"` is a value, not an error** (FR33, FR35). Existing logins cannot be retroactively classified. Reporting them as `"unknown"` with a "log in again to record it" label is honest and self-healing, and keeping `null` for the API-key case preserves the distinction between "no grant" and "a grant of unrecorded type."

- **Self-service verbs require a user credential and fail before calling the API otherwise** (FR41), over sending the request and relaying whatever the API says. "List my grants" is meaningless for an operator key or a pair, and the helpful response — log in, or pass `--org` — is Rover's to give, not the API's.

- **Not-found hides whether the grant exists** (FR52). A self-service caller probing grant IDs must not learn which ones belong to other users. The same message covers "never existed," "already revoked," and "someone else's," at the cost of a slightly less specific error.

- **Revoking the current grant is strict where `logout` is best-effort** (FR53). `rover auth logout` treats a failed server-side revocation as a warning and clears the local credential anyway, because its job is "stop using this here." `grants revoke` is explicitly a server-side action, so clearing the local credential after a failed revocation would leave the operator believing the grant is dead while it is live.

- **The per-user sweep prompts with the clients, not the grants, until grants can be listed** (FR57, FR82), resolving a tension in the PRD. B3.4 asks for the prompt to "print the grants that will be revoked," but B3.3 ships on shipped API alone, and nothing shipped can list a user's grants. The client list is what Rover actually knows and actually acts on, so it is what the prompt states; the grant list is added when it becomes truthful.

- **Rover's own OAuth client is named in the prompt as reaching every organization** (FR57), and recorded as an open question (§7). A user's personal browser and device-code logins are issued under Rover's single OAuth client, which is not registered to any one organization. Revoking a user's tokens under it therefore logs them out of Rover everywhere, not only for `acme`. The PRD's "only reaches clients registered under the target organization" is true for pairs and not for this client. Whether an admin of one organization should be able to do that is the Platform API's permission decision; this spec only requires that the prompt say so plainly.

- **A non-member target is a warning, not an error** (FR60). Offboarding commonly removes the membership first. Refusing to sweep a non-member would block exactly the runbook the sweep exists for, while silently sweeping one would let a mistyped user ID report success after revoking nothing. A warning, plus `user_is_member` in JSON, catches the typo without blocking the real case.

- **Success under a client means "nothing left," not "something revoked"** (FR62, FR64). The underlying call reports no count, so Rover cannot say how many grants it revoked and must not imply that it did. What it can say, and what a runbook needs, is that after the command no grant remains under that client. This is also what makes the command safe to re-run as its own retry mechanism.

- **Enumeration completes before any revocation starts** (FR56). A sweep that revoked under some clients and then discovered it could not see the rest would be a partial failure the user did not consent to; failing up front leaves the system unchanged.

- **`--confirm` is accepted on every revoke form** (FR49), over rejecting it where it has no effect. Offboarding and incident-response scripts are easier to write when one flag can be passed unconditionally, and accepting a redundant confirmation can never widen what a command does.

- **Unavailable verbs do not ship as stubs** (FR78, FR79). A `rover auth grants list` that always fails with "not yet supported" would be documented behavior the release does not implement. Parts ship when their backing capability does, and `--help` is the contract of what a given release offers.

---

## 7. Open questions

- **Can an organization admin revoke a user's grants under Rover's own OAuth client?** FR55 includes it and FR57 warns that it reaches every organization, but whether the Platform API's per-user revocation permits an admin of one organization to act on a client not registered to that organization is for the Platform API to decide. If it does not, the sweep's Rover-client entry will fail under FR63 on every run, and FR55 needs revisiting rather than the sweep being reported as permanently partial.
- **How does Rover learn a pair's `created_by` display name?** FR15 permits `null`; if the Platform API reports only an opaque actor ID, the text table shows the ID.

---

## 8. Test obligations

This contract cannot be verified by unit tests alone. The following must be closed alongside the work:

- **Snapshot coverage of every new JSON payload.** FR8, FR15, FR26, FR33, FR46, FR67, and FR68 are machine-readable contracts that runbooks will match on. They need snapshot coverage that fails on a renamed or retyped field, in both integration and end-to-end tests, rather than field-by-field assertions.
- **A no-regression snapshot of today's `rover api-key` output.** FR14 and FR83 promise that text output for API keys is unchanged. That needs the current output captured before the first slice lands, so the promise is checked against a fixed baseline rather than against itself.
- **Secret-leak assertions at trace log level.** FR69–FR71 need a test that runs create and rotate with maximum log verbosity and fails if the secret appears anywhere but stdout, including in the failure path of FR71.
- **Per-client failure injection for the sweep.** FR61–FR64 require a fixture in which revocation fails under one client and succeeds under the rest, and in which enumeration fails partway through (FR56), so that "attempts every client," "exits non-zero," "names the failures," and "revokes nothing if enumeration fails" are each observable.
- **Non-terminal stdin.** FR59 needs a test that runs the sweep with stdin redirected and no `--confirm`, on every supported platform, because what counts as a terminal differs between Unix and Windows.
- **Cross-version credential compatibility.** FR36 needs a test that writes a credential with this version of Rover and reads it with the most recent release that did not record grant type.
