---
category: feat
breaking: false
authors: [dotdat]
---

`rover api-key rotate` mints a new secret for a client-credential pair

`rover api-key rotate <ORGANIZATION_ID> <CLIENT_ID> [--grace-period-days <DAYS>]` rotates a pair's secret, printing the new client ID/secret/expiry to stdout (so CI setup can capture them) and a one-time reminder to stderr that the secret can't be shown again, alongside a warning naming when every previous secret stops working - immediately by default, or at the end of `--grace-period-days` when given. `--format json` reports `client_id`, `client_secret`, `secret_expires_at`, `grace_period_days`, and `previous_secrets_expire_at` under `key_type: "ClientCredentials"`. Rotating an ID that isn't a client-credential pair in that organization fails with a new, stable error code (`E057`) rather than silently doing nothing.
