---
category: feat
breaking: false
authors: [dotdat]
---

`rover api-key list` reports client-credential pairs alongside API keys

`rover api-key list <ORGANIZATION_ID>` now also lists the organization's client-credential pairs, in a `Client-credential pairs` table after the existing API key table. `--format json` adds a `client_credentials` array (`key_type: "ClientCredentials"`, `client_id`, `name`, `graphs`, `scopes`, `created_at`, `created_by`) and a `key_type` field on every existing `keys` entry. A new `--type <operator|subgraph|client-credentials>` flag (repeatable) narrows which types are reported; a type excluded entirely is omitted from the JSON payload rather than reported as `[]`. Existing `keys`-only output is unchanged when an organization has no pairs, or when `--type` excludes them.

Pairs are paged and capped independently of API keys: by default, up to 100 are collected before returning, and a new `--limit <N>` flag overrides that cap. When more pairs exist beyond the cap, the command still succeeds, printing a resume note (and a JSON `client_credentials_next_after` cursor) that a new `--after <CURSOR>` flag accepts to continue from.
