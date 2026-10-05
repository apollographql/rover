---
category: feat
breaking: false
authors: [dotdat]
---

Add `rover auth logout`, gated behind the experimental `oauth` feature flag

`rover auth logout` revokes the OAuth session stored by `rover auth login` for the given `--profile` (or "default") — the access token and, if one was issued, the refresh token (RFC 7009) — then removes the local credential. Revocation is best-effort: if the OAuth server can't be reached, Rover still clears the local credential and warns instead of leaving you stuck "logged in" locally. Only meaningful for profiles logged in via `rover auth login`; running it against a profile holding a Personal API Key (from `rover config auth`) errors and points you at `rover config delete` instead. Only compiled in when built with `--features oauth`, matching `rover auth login`.
