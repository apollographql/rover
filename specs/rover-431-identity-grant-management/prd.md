# PRD: Rover Identity & OAuth Grant Management

This PRD covers two related but independently shippable capabilities that share a problem statement and a permission model. **Part A** (client credential management) gives org admins a first-class, org-scoped machine identity for CI that they can create, list, rotate, and delete from Rover. **Part B** (OAuth grant management) gives individual developers visibility into and control over their own active grants, and gives org admins and security leads org-wide visibility plus the ability to revoke any single grant or every grant belonging to one user, for offboarding and incident response. Rover is the consumer in both parts. Part A is backed by Platform API fields that already exist; Part B is blocked on new grant-enumeration and revocation capability owned by another team (§9).

## 1. Problem statement

Rover can now authenticate three ways: a stored or environment-supplied API key, an interactive OAuth login (browser or device code), and a non-interactive OAuth client-credentials exchange for CI. But once a credential exists, Rover offers almost nothing for managing it:

1. **CI has no first-class machine identity.** The client-credentials path exists in Rover, but there is no way to obtain, list, rotate, or delete a client ID/secret pair from Rover. Teams that want to move CI off long-lived personal or graph API keys have no self-service path to the credential the new flow expects, and no way to scope it to just the graphs a pipeline touches.
2. **Nobody can see their active grants.** A developer who logged in from a laptop, a build box over SSH, and a device-code flow on a headless machine has no way to enumerate those grants or tell which is which. The only revocation available is `rover auth logout`, which revokes the *current* profile's grant and nothing else. A lost laptop means either living with a live token or logging out everywhere.
3. **Org admins have no incident-response or offboarding controls.** There is no way to list every client-credentials pair or active grant in an organization, revoke one suspicious grant without forcing a company-wide re-login, or revoke every grant belonging to one departing user. The only bulk mechanism in GraphOS today targets Studio web-login sessions, which are a different resource, and it is blind (all-or-nothing, no listing, no targeting).

The underlying gap is on the identity server: it can issue grants and bulk-invalidate them per client, but it cannot enumerate them or revoke them individually. The stored grant data already records the identifiers needed for per-grant visibility; nothing reads them today. This PRD establishes what Rover needs from that capability once it exists, and what Rover exposes on top of it.

## 2. Background: how this works today

### 2.1 Rover

- **Credential resolution** prefers the `APOLLO_KEY` environment variable, then a client-credentials exchange when both `APOLLO_CLIENT_ID` and `APOLLO_CLIENT_SECRET` are set, then the credential stored in the selected profile. Setting only one of the two client-credentials variables is an error.
- **`rover auth login|logout|whoami`** is real, working code: browser PKCE flow, `--no-browser` device-authorization flow, token revocation on logout, and identity lookup. It is currently compiled only when the `oauth` Cargo feature is enabled, and that gate is expected to be removed soon. Part B extends this command family.
- **`rover auth whoami` and `rover config whoami`** already report an *origin* for the active credential: the `APOLLO_KEY` env var, a profile's stored API key, a profile's OAuth login, or the `APOLLO_CLIENT_ID` env var. Origin does **not** distinguish a browser login from a device-code login; both are stored as the same kind of OAuth credential. Neither command can describe any grant other than the one currently in use.
- **`rover api-key create|list|delete|rename`** manages organization API keys (`operator` and `subgraph` types), addressed by organization ID and key ID, with per-subgraph scoping supplied as a YAML document or on stdin. It is the closest existing shape for an org-scoped, named credential resource, but it has no `rotate` verb for any key type and knows nothing about OAuth clients.
- **Organization API keys already support rotation with an overlap window** on the Platform API (new key issued, old key expires at a caller-supplied time, one day by default). Rover does not expose it. Organization and graph API keys also already report a last-used timestamp.
- **`rover config auth|list|delete|clear|whoami`** is the legacy API-key profile path. It is not the home for either new capability.

### 2.2 GraphOS Platform API

Verified against the identity service on the monorepo's main branch as of 2026-09-24.

- **Client-credentials CRUD already exists in the `platform-api` contract**, on the organization query and mutation types rather than under a separate `oauth` namespace:
  - `oauthClients(first, after)` lists an organization's clients with forward-only pagination and a total count, and `oauthClient(clientId)` fetches one. Each client exposes its ID, name, creation time, creating actor, grant types, graph resources, and scopes. Neither carries a secret or a last-used time.
  - `createOAuthClient(clientName, resources, scopes, secretLifetimeDays)` registers a client for the `client_credentials` grant and returns it with the secret, which is never returned again. `resources` names one to fifty graphs, all of which must belong to the organization. `scopes` must be non-empty and, today, may only contain `rover:cli`. The secret lives two years by default and at most 730 days.
  - `rotateOAuthClientSecret(clientId, gracePeriodDays)` mints a new secret and expires every other secret the client holds after the grace period. **The default grace period is zero**: without the argument, the old secret stops working immediately. The maximum is thirty days.
  - `deleteOAuthClient(clientId)` soft-deletes the client together with its secrets, service account, and principal. New tokens can no longer be minted, but an access token already issued keeps working until it expires naturally, up to fifteen minutes.
  - `revokeUserOAuthTokens(clientId, userId)` revokes all of one user's refresh tokens under one OAuth client. It does not revoke outstanding access tokens and does not stop the user re-authenticating.
- **Permissions on a client come from its scopes, not from a role.** The `rover:cli` scope, the only one an admin can register today, grants a fixed capability set over the named graphs: read the organization, graph, and variants; read schemas and implementing services; run schema checks; create variants; and publish schemas and subgraphs. There is no per-client role selector and nothing equivalent to the graph API key roles. Scope narrows within the organization and can never widen back to it.
- **Every mutation is org-admin only and gated the same way.** Registering requires the org-admin role, and rotate and delete have their own permissions at the same level. All of them are behind a per-actor feature flag for the initial rollout and require the same plan tier as subgraph and variant API keys.
- **Every state change is already audited.** Creating a client writes org-scoped audit rows for the client, its service account, its principal, and its first secret, each recording the acting identity. Rotation writes a row for the new secret and one for every secret it expires. Deletion writes rows for the client, its secrets, and its principal. Reads are not audited. Organization admins can already pull these rows through the Platform API's audit export.
- **There is still no GraphQL exposure for enumerating OAuth grants or revoking a single grant.** The refresh-token store can revoke per user per client (above) or everything for a client, but cannot list grants or target one.
- Rover's "Platform API" is a contract variant of the same federated graph the identity service participates in, so any further field is additive schema on the existing identity subgraph, not a new service.
- The existing `revokeUserSessions` mutation targets Studio browser-login sessions, a different resource with its own management service. It must not be conflated with OAuth grants.

### 2.3 Terminology

**Grant** means the OAuth 2.0 grant issued to a user or workload: the tokens resulting from an authorization-code, device-code, or client-credentials exchange. This PRD uses "grant" throughout, in prose, CLI verbs, and the Platform API dependencies, and deliberately avoids "session", because GraphOS already uses that word for Studio web logins, which have their own management and revocation feature.

**Client credential pair** means an OAuth 2.0 `client_id`/`client_secret` registered under an organization for the client-credentials grant. It is a distinct resource from an organization API key, even though this PRD proposes managing both through `rover api-key`.

## 3. Permission model

Rover recognizes two tiers. Which tier a caller is in is decided by the Platform API on every request; Rover never evaluates ownership or role itself, it only surfaces the server's decision as a clear permission error.

| Tier | Client credential pairs | OAuth grants |
|---|---|---|
| **Org admin / security lead** | Create, list, rotate, delete any pair in the org | List every grant in the org; revoke any single grant; revoke every grant belonging to one user |
| **Individual developer** | None | List own grants; revoke own grants one at a time |

Four personas motivate the requirements, and they map onto these tiers as follows. The *individual developer* is the developer tier. The *org admin / security lead* is the admin tier. The *CI / platform engineer* who sets up a pipeline's credential does so as an admin; this PRD does not define a narrower "pairs I created" scope, and if the Platform API later offers one, Rover surfaces it without changing any verb. The *support engineer* is out of scope for Rover (§5).

Three rules follow:

- **OAuth grants support only Read and Delete.** A grant is a side effect of authenticating; there is no "create a grant" or "edit a grant" action. Client credential pairs support create, read, update (rotate), and delete.
- **Self-service and org-wide are separate capabilities, not one capability with a wider filter.** Org-wide visibility carries meaningfully higher blast radius and is what the security-lead persona needs for incident response. It gets its own permission on the Platform API side and its own explicit flag on the Rover side, so a user can never widen scope by accident.
- **There is no org-wide bulk revoke** (§5).

## 4. Goals

1. **An admin can stand up CI authentication end to end from Rover**, with no personal login and no static `APOLLO_KEY`: create a client-credential pair scoped to the graphs the pipeline needs, receive the secret exactly once, and use it via `APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET` in the same session.
2. **Routine secret rotation can be done without breaking in-flight CI.** Rotating a pair yields a new secret, and the operator chooses a grace period during which the previous secret keeps working.
3. **Cleanup is informed, not guesswork.** Listing pairs shows when each was created and last used, so unused credentials can be deleted with confidence.
4. **A developer can see every active grant tied to them, labeled by how it was established** (browser login, device code, client credentials), and revoke exactly one without disturbing the others.
5. **An org admin can list every client-credential pair and every active grant in the org**, revoke any single grant, and revoke every grant belonging to one user, with confirmation for the per-user case.
6. **Machine-readable output for every verb**, through Rover's standard JSON envelope, so the create/rotate/list/revoke flows can be scripted in CI and incident-response runbooks.
7. **No regression for existing users.** `rover api-key` for `operator`/`subgraph` keys, `rover auth login|logout|whoami`, and the `APOLLO_*` credential env vars keep their current behavior. Everything here is additive.

## 5. Non-goals

- **Org-wide bulk revocation.** No verb, flag, or Platform API dependency in this PRD revokes every grant in an organization. Per-user revocation (offboarding) and single-grant revocation (a suspicious login) cover every scenario in §1; a leaked client-credential pair is handled by deleting the pair (Part A), which invalidates its grants.
- **Support-engineer tooling.** A support engineer diagnosing a customer's login problem wants a cross-org presence check (does this user have an active grant?) or status check (is this pair valid or revoked?), without seeing the credential and without any revoke authority. That belongs in internal tooling, not Rover. It is recorded here so that granting Rover any cross-org read is recognized as scope creep, not an oversight.
- **Studio UI.** Client-credential CRUD is expected to exist in both Studio and Rover. This PRD covers only the Rover surface. Studio work is tracked elsewhere.
- **Studio web-login session management.** Existing bulk revocation of Studio browser sessions is a different resource and is not touched.
- **The identity-server storage and Platform API design itself.** §9 records what Rover depends on, at the level of capability and proposed field, so the dependency is legible. The design of those fields, their permissions, and their storage is owned by the Platform API team.
- **Deprecating API keys or `rover config`.** Nothing here removes or discourages existing credential types beyond the advisory `rover config whoami` already prints.
- **Rotating `operator`/`subgraph` API keys.** `rotate` is introduced only for client-credential pairs in this PRD; extending it to other key types is a natural follow-on but not addressed here.

## 6. Part A: Client credential management

Extends `rover api-key`, which already models an org-scoped, named credential with create/list/delete verbs, rather than introducing a new top-level noun. A client-credential pair appears as a third key type alongside `operator` and `subgraph`. Every verb below is backed by a Platform API field that already exists (§2.2); Part A is consumer work.

### A1. Requirements

1. **Create.** `rover api-key create <ORGANIZATION_ID> client-credentials <NAME> --graph-id <GRAPH_ID>...` registers a new pair under the organization and prints the `client_id` and `client_secret`.
   - The secret is shown **exactly once**, at creation. No later command can retrieve it. The output must say so, and must show when the secret expires.
   - **Scope is a set of graphs.** `--graph-id` is required and repeatable, naming between one and fifty graphs that belong to the organization. The pair's permissions over those graphs are fixed by the `rover:cli` scope (§2.2): read, check, and publish, which is what a Rover pipeline needs. Rover always requests that scope and exposes no scope or role selector, because the API accepts nothing else today. If the API later admits more scopes, Rover adds a flag then.
   - `--secret-lifetime-days <N>` optionally shortens the secret's lifetime. The API's default is already its 730-day (two-year) maximum, so this flag can only ever shorten it, never lengthen it. The API does not document a minimum; Rover does not enforce one client-side and surfaces whatever validation error the API returns for an out-of-range value.
   - JSON output carries `client_id`, `client_secret`, `secret_expires_at`, and the graph list as distinct fields so CI setup can be scripted without parsing prose.
2. **List.** `rover api-key list <ORGANIZATION_ID>` includes client-credential pairs alongside other key types, showing at minimum name, type, `client_id`, graphs, creation time, and who created it. Secrets never appear in list output. Rover pages through the API's forward-only cursor so the user sees the whole set. `rover api-key list` must accept an optional `--type` filter so an admin can audit machine identities alone. Last-used time is shown once the API exposes it (§9).
3. **Rotate.** `rover api-key rotate <ORGANIZATION_ID> <CLIENT_ID> [--grace-period-days <N>]` mints a new secret, prints it once with its expiry, and expires every other secret the pair holds after the grace period. **Rover mirrors the API default: with no flag, the old secret stops working immediately.** The output states when the old secret expires so a hard cutover is never silent, and the docs recommend a grace period for zero-downtime rotation. The maximum grace period is thirty days. Name and graphs carry over unchanged.
4. **Delete.** `rover api-key delete <ORGANIZATION_ID> <CLIENT_ID>` deletes the pair. The pair can no longer obtain tokens, but an access token already issued keeps working until it expires, up to fifteen minutes; the output says so. Existing delete semantics (irreversible, confirm the ID) carry over. This is the response to a leaked pair.
5. **No rename.** The API has no rename for OAuth clients, so `rover api-key rename` rejects a client-credentials ID with a clear message until one exists (§9).
6. **Admin-only.** Every Part A verb requires the org-admin role (§3), and during the initial rollout the organization must also be enrolled in the feature flag. Rover does not enforce either itself; it surfaces the Platform API's decision with a clear permission error.

### A2. Key decisions and rationale

- **`rover api-key`, not a new noun.** The verbs (create/list/delete), the addressing (organization ID, then a per-key ID), and the audience already exist. Adding a key type is the smallest change that gives the capability a discoverable home, and it keeps "things an org admin issues to machines" in one place. The costs are that a pair has two values where an API key has one, and that the API models OAuth clients as a separate resource from API keys, so `list` merges two queries. Both are Rover's problem to hide, not the user's.
- **Graph set plus fixed scope.** This follows the shipped API rather than the graph API key model. A pipeline that touches several graphs gets one pair instead of several, and the permission set is the one Rover's own commands need. The trade is that there is no read-only or narrower pair today; that arrives when the API admits more scopes, and Rover will follow.
- **`rotate` is a new verb, introduced for client credentials first.** No key type has it in Rover today, although the API already supports it for organization keys.
- **Mirror the API's immediate-by-default rotation.** Studio and any other Platform API client will get the same default, so a Rover-specific overlap default would make the same action behave differently by surface. Rover compensates by always printing the old secret's expiry and by documenting `--grace-period-days` as the zero-downtime path.
- **Show-once secrets.** Matches the precedent of every comparable CLI and the API's own contract.
- **Audit trail needs no Rover work.** Every create, rotate, and delete is already recorded server-side with the acting identity and exported through the organization's existing audit export (§2.2). Rover's job is to pass through the caller's identity, which it does by authenticating normally.

## 7. Part B: OAuth grant management

Extends `rover auth`, the home of the interactive OAuth flows, with a `grants` noun.

### B1. Requirements: current grant

1. **`rover auth whoami` states the mechanism that established the current grant**: browser login, device code, or client credentials, in addition to the identity and origin it reports today. Today browser and device-code logins are indistinguishable in the stored credential, so this requires Rover to record the grant type at login time.

### B2. Requirements: own grants (self-service)

1. **List.** `rover auth grants list` shows every active grant belonging to the caller: browser logins and device-code logins. Client-credentials grants belong to the client, not to a person, and appear only in the admin's org-wide list (B3.1). Each row shows at minimum a stable grant identifier, how it was established, when it was created, when it was last used or refreshed, and whichever of those grants is the one the current invocation is using.
2. **Label headless grants clearly.** A grant established via `--no-browser` on a build box must be visibly distinct from a laptop browser login, so a developer deciding what to revoke does not confuse the two.
3. **Revoke one.** `rover auth grants revoke <GRANT_ID>` revokes exactly that grant. Other grants belonging to the same user stay valid. Revoking the grant the current invocation is using is allowed and behaves like `rover auth logout` for that profile.
4. **Zero-permission bar.** Any authenticated member can list and revoke their own grants; no org role is required.

### B3. Requirements: org-wide grants (admin / security lead)

1. **List.** `rover auth grants list --org <ORGANIZATION_ID>` shows every active grant in the organization across every member and every client-credential pair, with the same per-grant fields as B2 plus the principal (user or client) each grant belongs to. This is a first-class capability, not a filter on the self-service list, because "who or what had an active grant during the breach window" is the incident-response question.
2. **Revoke one, any member's.** `rover auth grants revoke --org <ORGANIZATION_ID> <GRANT_ID>` revokes one specific grant belonging to any member of the organization (one suspicious login), leaving every other grant untouched.
3. **Revoke all of one user's, across every OAuth client.** `rover auth grants revoke --org <ORGANIZATION_ID> --user <USER_ID> --all [--confirm]` revokes that user's grants under every OAuth client Rover can see: its own static client (personal browser/device-code logins) and every client-credential pair in the organization, discovered by paging through `oauthClients(first, after)` (§9) the same way A1.2's list does. Rover calls the shipped `revokeUserOAuthTokens(clientId, userId)` once per client; a call against a client the user never had a grant under is a harmless no-op. This still does not touch Studio web-login sessions, a different resource with its own revocation (§5), and it only reaches clients registered under the target organization — a user's grants under a *different* organization's client-credential pairs are untouched. `--all` is only valid together with `--user`; it is the widest revocation Rover offers.
4. **Confirmation for the per-user case.** Revoking one grant needs no prompt. Revoking all of a user's grants prints the grants that will be revoked and asks for a yes/no confirmation that defaults to no; declining prints a cancellation notice and exits successfully without revoking anything. Passing `--confirm` skips the prompt for scripted offboarding. This is the same shape `rover graph delete` and `rover subgraph delete` use today for their irreversible actions, and the flag name is reused deliberately. Rover must never treat `--user` without `--all` as "revoke everything for this user"; the flag is required so the wider action is always spelled out on the command line.
5. **Report per-client outcome.** Because the per-user sweep (B3.3) is one `revokeUserOAuthTokens` call per OAuth client rather than a single atomic mutation, the output must say which clients were revoked successfully and which failed, by name/ID, rather than a single pass/fail for the whole command. A partial failure (e.g. the API call for one client-credential pair errors) must exit non-zero and name exactly which client(s) still need a retry, so an offboarding runbook never reports success while a grant is still live.
6. **No org-wide revoke.** There is no flag or argument combination that revokes every grant in an organization. `--all` without `--user` is an error.
7. **Separate permission.** Org-wide list and revoke require the admin/security-lead permission on the Platform API side. A member without it gets a clear permission error, not an empty list.

### B4. Key decisions and rationale

- **Two tiers, two flags.** `--org` is an explicit opt-in to the wider scope on every verb. A developer can never accidentally list or revoke beyond their own grants by omitting a filter, and an admin's org-wide action is visible in shell history and runbooks.
- **Per-user is the ceiling.** Incident response needs "one suspicious grant" and offboarding needs "everything for one departing user." Nothing needs "everything in the org," and offering it would give a single mistyped command the power to force a company-wide re-login. A compromised client-credential pair is handled by deleting the pair, not by a grant-level bulk revoke.
- **Grant, not token.** The unit a user lists and revokes is the grant (the authorization that produced the tokens), not an individual access or refresh token. Revoking a grant invalidates everything derived from it.
- **`rover auth logout` stays as-is.** It remains the fast path for "revoke my current grant on this machine." `grants revoke` is the general path.
- **`--confirm`, not a new flag.** Rover already has one convention for "skip the prompt before an irreversible action," on graph and subgraph delete. Offboarding scripts should learn one flag, and the docs can describe it once.
- **Enumerate clients and loop, rather than wait on a server-side fan-out mutation.** `revokeUserOAuthTokens` only ever revokes one client at a time, and there is no Platform API mutation that revokes a user across every client in one call. Rover already has everything it needs to approximate that itself: `oauthClients(first, after)` (shipped, used by A1.2) enumerates every client-credential pair in the organization, and Rover knows its own static client ID. Looping — Rover's client, then every paginated result from `oauthClients` — ships B3.3 now instead of behind a new Platform API dependency, at the cost of N sequential calls instead of one atomic one, which is why B3.5 requires per-client outcome reporting.

## 8. Cross-cutting requirements

1. **Standard output envelope.** Every verb supports `--format json` through Rover's existing `{"json_version", "data", "error"}` envelope. `data` is self-describing (typed fields, not prose), and permission failures surface as a distinct `error.code`.
2. **Secrets never reach logs.** Create and rotate print the secret to stdout once. Debug logging, telemetry, and error messages never include a secret or a full token.
3. **Documentation.** `rover api-key` docs gain the `client-credentials` type and the `rotate` verb, with a worked CI setup example that ends in `APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET`. `rover auth` docs gain the `grants` verbs and the two-tier permission model. Docs ship in their own PR.
4. **Changelog entries** for each user-visible slice.

## 9. Dependencies on the Platform API

Rover is the consumer. Part A's fields exist today; Part B's mostly do not. Field names below are the shipped ones where they exist and a proposed shape, following the terminology in §2.3, where they do not.

| Rover capability | Platform API capability | Status |
|---|---|---|
| A1.1 create pair | `createOAuthClient(clientName, resources, scopes, secretLifetimeDays)` | Shipped |
| A1.2 list pairs | `oauthClients(first, after)`, `oauthClient(clientId)` | Shipped |
| A1.3 rotate secret | `rotateOAuthClientSecret(clientId, gracePeriodDays)` | Shipped |
| A1.4 delete pair | `deleteOAuthClient(clientId)` | Shipped |
| A1.5 rename pair | none | Gap |
| B2.1 list own grants | `myGrants` | Proposed |
| B2.3 revoke own grant | `revokeGrant(grantId)` | Proposed |
| B3.1 list org grants | `grants(accountId)` | Proposed |
| B3.2 revoke any member's grant | `revokeGrant(grantId)` with org permission | Proposed |
| B3.3 revoke all of one user's grants | `oauthClients(first, after)` to enumerate, then `revokeUserOAuthTokens(clientId, userId)` per client | Shipped |

Remaining gaps Rover needs resolved before the corresponding requirement can be met in full:

- **Last-used time** on an OAuth client (A1.2) and on a grant (B2.1). API keys already expose one; the client record and the grant record need the same.
- **Rename** for OAuth clients (A1.5).
- **Grant enumeration and single-grant revoke** (B2, B3.1, B3.2): the refresh-token store records the identifiers needed but nothing reads them yet. This is the central gap behind Part B.
- **Grant type on a grant** (B1, B2.2): the grant record must expose whether it came from an authorization-code, device-code, or client-credentials exchange. The API already has the enum for this on the client record.
- **No org-wide revoke on the API either.** The shipped `revokeUserOAuthTokens` requires a user, and any new grant-revocation mutation should be equally explicit, never treating an omitted target as "everything in the org," so no client of the API can wipe an org by omitting an argument.

## 10. Success metrics

- An org admin authenticated with an `operator` API key can run `rover api-key create ... client-credentials ...`, export the two printed values, and successfully run `rover subgraph publish` against a scoped graph with no `APOLLO_KEY` set, in one sitting and without reading source.
- After `rover api-key rotate --grace-period-days 1`, a job using the old secret started before rotation completes successfully, and a job started after the grace period with the old secret fails with a clear authentication error. After `rover api-key rotate` with no flag, the output states that the old secret has expired.
- `rover auth grants list` for a user logged in from a browser and from `--no-browser` on a second machine shows two grants with distinguishable labels; revoking one leaves the other working.
- `rover auth grants revoke --org ... --user ... --all` for a departing user revokes every one of their grants and leaves every other member's grants valid.
- `rover auth grants revoke --org ... --all` without `--user` exits non-zero with a usage error and revokes nothing.
- Every verb's `--format json` output is consumable with `jq` on the documented `data` fields; permission denials are distinguishable by `error.code`.
- `rover api-key` for `operator`/`subgraph` keys and `rover auth login|logout|whoami` behave exactly as before.
