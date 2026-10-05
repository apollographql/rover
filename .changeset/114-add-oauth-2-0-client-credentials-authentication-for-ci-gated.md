---
category: maint
breaking: false
authors: [dotdat]
---

Add OAuth 2.0 client credentials authentication for CI, gated behind the experimental `oauth` feature flag

If `APOLLO_CLIENT_ID` and `APOLLO_CLIENT_SECRET` are both set, Rover now exchanges them for an access token via the OAuth 2.0 client credentials grant (RFC 6749 §4.4) and uses it exactly as it would an `APOLLO_KEY` — no new subcommand, no stored session, no other behavior change. This is meant for CI/machine-to-machine use where the interactive, browser-based `rover auth login` isn't an option. Precedence is unambiguous: `APOLLO_KEY` still always wins; if it's unset and both client-credentials env vars are present, the exchanged token is used instead; otherwise Rover falls back to whatever a stored profile already resolves to, unchanged. Setting only one of `APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET` is treated as a configuration error rather than silently falling through to a stored profile. The exchange reuses the same OAuth token endpoint `rover auth login` does (override with `--oauth-token-url`). Only compiled in when built with `--features oauth`, matching `rover auth login`/`logout`/`whoami`.
