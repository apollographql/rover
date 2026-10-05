---
category: maint
breaking: false
authors: [dotdat]
---

Add `rover auth whoami`, gated behind the same experimental `oauth` feature flag

`rover auth whoami` displays the identity of the currently authenticated profile. For a profile logged in via `rover auth login`, it queries the OAuth identity provider's `/userinfo` endpoint directly and shows the account's name, email, and user ID; for a profile still using a legacy Personal API Key (via `rover config auth` or `APOLLO_KEY`), it falls back to the same Apollo Studio lookup `rover config whoami` already does. `rover config whoami` itself is unchanged aside from a new stderr note pointing at `rover auth whoami` going forward. Both lookups now go through a `tower` retry/timeout policy (bounded per-attempt timeout, exponential-backoff retry on transient failures) — the OAuth REST call didn't have either before, so a hung connection or a flaky identity provider could previously leave the command stuck indefinitely. The new `--oauth-whoami-url` flag overrides the `/userinfo` endpoint the same way the existing OAuth endpoint flags do.
