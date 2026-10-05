---
category: maint
breaking: false
authors: [dotdat]
---

Add `rover auth login`, gated behind an experimental `oauth` feature flag

`rover auth login` authenticates via OAuth 2.0 (PKCE authorization-code flow): it opens your browser, completes the login against Apollo's Identity service, and stores the resulting session the same way `rover config auth` stores a Personal API Key (`--profile <name>` works the same way). Only compiled in when built with `--features oauth` — off by default, and not part of any released binary yet. Uses a static, pre-registered OAuth client (one per environment) rather than registering a new client per install; the top-level `--oauth-authorization-url`/`--oauth-token-url`/`--oauth-client-id` flags override the defaults (Apollo's production OAuth server and its registered `rover` client) for testing against other environments. These are top-level flags, not ones scoped to `auth login`, so they'll also apply to any future command that needs to refresh an OAuth token. The authorization URL is always printed to stderr regardless of whether a browser opens; pass `--no-open` to skip the open attempt entirely — useful over SSH or in any environment without a browser.
