---
category: feat
breaking: false
authors: [dotdat]
---

`rover api-key delete` deletes client-credential pairs

`rover api-key delete <ORGANIZATION_ID> <ID>` now accepts a pair's client ID as well as an API key's ID - no flag picks the type; Rover looks the ID up as a pair first. Deleting a pair confirms on stderr that it can no longer obtain tokens and that access tokens it already holds keep working until they expire, and `--format json` reports `key_type: "ClientCredentials"` alongside the existing `id`. Deleting an API key is unchanged, including for callers who can't see the organization's pairs.
