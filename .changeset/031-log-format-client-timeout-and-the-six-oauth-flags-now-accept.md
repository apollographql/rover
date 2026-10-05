---
category: feat
breaking: false
authors: [dotdat]
---

`--log`, `--format`, `--client-timeout`, and the six `--oauth-*` flags now accept an environment variable equivalent

`--log`/`APOLLO_LOG_LEVEL`, `--format`/`APOLLO_FORMAT`, `--client-timeout`/`APOLLO_CLIENT_TIMEOUT`, `--oauth-authorization-url`/`APOLLO_OAUTH_AUTHORIZATION_URL`, `--oauth-token-url`/`APOLLO_OAUTH_TOKEN_URL`, `--oauth-whoami-url`/`APOLLO_OAUTH_WHOAMI_URL`, `--oauth-revocation-url`/`APOLLO_OAUTH_REVOCATION_URL`, `--oauth-device-authorization-url`/`APOLLO_OAUTH_DEVICE_AUTHORIZATION_URL`, and `--oauth-client-id`/`APOLLO_OAUTH_CLIENT_ID` were previously flag-only. The flag wins when both are set; each flag's `--help` text now names its environment variable. No existing behavior changes: this is purely additive, part of ROVER-451's Prerequisite slice giving every global setting both a flag and an environment variable.
