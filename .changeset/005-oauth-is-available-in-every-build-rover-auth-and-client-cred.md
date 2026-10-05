---
category: feat
breaking: false
authors: [dotdat]
---

OAuth is available in every build: `rover auth` and client-credentials authentication

The experimental `oauth` Cargo feature is removed, and what it gated now ships in every build:

- `rover auth login`, using the browser or `--no-browser`, plus `rover auth logout` and `rover auth whoami`
- `rover auth grants revoke`, which revokes every grant one user holds in an organization
- authenticating with a client-credential pair by setting `APOLLO_CLIENT_ID` and `APOLLO_CLIENT_SECRET`
- the `--oauth-*` flags and their `APOLLO_OAUTH_*` settings, which now appear in `rover config show`

`rover config auth` and `rover config whoami` now point out that `rover auth login` is available. Building with `--features oauth` is no longer needed, and no longer accepted.
