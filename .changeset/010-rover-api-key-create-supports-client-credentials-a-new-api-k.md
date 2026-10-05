---
category: feat
breaking: false
authors: [dotdat]
---

`rover api-key create` supports `client-credentials`, a new API key type for CI setup

`rover api-key create <ORGANIZATION_ID> client-credentials <NAME> --graph-id <GRAPH_ID>... [--secret-lifetime-days <DAYS>]` registers an OAuth 2.0 client-credentials pair scoped to the named graphs, requesting exactly the `rover:cli` scope. On success it prints the client ID and secret to stdout (so CI setup can capture them) and a one-time reminder to stderr that the secret can't be shown again; `--format json` reports `client_id`, `client_secret`, `secret_expires_at`, `name`, `graphs`, and `scopes` under `key_type: "ClientCredentials"`. The pair is usable immediately by setting `APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET` to the reported values. Existing `operator`/`subgraph` behavior is unchanged.
