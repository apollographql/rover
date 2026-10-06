---
category: fix
breaking: false
authors: [dotdat]
---

`rover graph publish` and `rover subgraph publish --format json` keep the publish response when the launch wait times out

When either command stopped waiting for the launch it triggered, it failed with E065 and `data` held nothing but `"success": false`, losing `api_schema_hash` and the rest of the response though the schema was published. `data` now carries the publish response, as it already does for a failed launch (E047), with `launch_status` `null` because Rover stopped waiting before it learned the outcome.
