---
category: feat
breaking: false
authors: [dotdat]
---

`rover auth whoami` reports how the current grant was established, gated behind the experimental `oauth` feature flag

Gains a `Grant Type` row (`grant_type` in JSON): `Browser login`/`"authorization_code"` and `Device code (--no-browser)`/`"device_code"` for a profile logged in via `rover auth login`, `Client credentials`/`"client_credentials"` for an `APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET` exchange, or `Unknown — log in again to record it`/`"unknown"` for an OAuth login stored by an older Rover version that didn't record it. Omitted from text output (and `null` in JSON) for a Personal API Key, which has no grant at all. `rover auth login` now records which flow it used; `rover config whoami`'s output is unchanged.
