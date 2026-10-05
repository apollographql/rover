---
category: feat
breaking: false
authors: [dotdat]
---

Add `--no-browser` to `rover auth login`, gated behind the experimental `oauth` feature flag

`rover auth login --no-browser` uses the OAuth 2.0 Device Authorization Grant (RFC 8628) instead of the local browser/redirect-server flow: it prints a verification URL and code to enter from any device, then polls until you approve the request, for headless or browser-less environments. Ignores `--no-open`, since there's no local browser step to skip. The device authorization endpoint can be overridden with `--oauth-device-authorization-url`, matching the other OAuth endpoint overrides.
