---
category: feat
breaking: false
authors: [dotdat]
---

Report triggered downstream contract-variant launches on `rover graph publish`

`rover graph publish` now reports which contract variants had a downstream launch triggered by the publish, with links to each — in text ("Triggered downstream launches for N contract variant(s): ...", printed to stderr) and as a `downstream_launches` JSON array alongside a `launch_url` field. Rover polls the publish's launch (and its downstream launches) to completion before reporting success, and fails the publish if the launch itself or any downstream launch didn't complete successfully. Nothing new prints when the publish triggered no downstream launches. stdout is unchanged for now (still just the schema hash on success) — a printed note warns that a future version will move this report into stdout, and points scripts that only need the hash at `--format json`'s `.data.api_schema_hash`. Note: like other Studio launch data fetched after the fact, this can race a concurrent publish to the same variant — a known limitation, not solved here.
