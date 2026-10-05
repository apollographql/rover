---
category: maint
breaking: false
authors: [dotdat]
---

Add a `rover-client` operation to look up one client-credential pair by its client ID

`pair_get::GetOAuthClient` looks up a single pair via `Organization.oauthClient`, reporting the Platform API's deliberately uninformative `null` (no such pair, no permission, or not enrolled) as `None` - the "not a pair, or can't tell" signal `rover api-key delete`/`rename` need to fall back to treating an ID as an API key. An HTTP 403 on the lookup is reported as `PairPermissionDenied`, which callers treat as "can't tell" too: `studio_graphql_service_with_attempt_timeout`, whose only callers are the pair reads, now classifies 403s the way `studio_graphql_service_with_timeout` already does. Also fixes `pair_delete` so a permission denial from `deleteOAuthClient` is reported as `E053`, matching `pair_create`. Not yet consumed by any command - foundation for rover-431's pair-aware `delete`/`rename`.
