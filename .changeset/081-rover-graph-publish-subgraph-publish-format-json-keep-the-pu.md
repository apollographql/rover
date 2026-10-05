---
category: fix
breaking: false
authors: [dotdat]
---

`rover graph publish`/`subgraph publish --format json` keep the publish response when a triggered launch fails

A failed launch or downstream contract-variant launch used to make either command return a bare error, discarding the whole response — `--format json` reported `"data": null` with no `error.code` to match on, even though the schema publish itself had succeeded. Both now surface this as `RoverClientError::PublishLaunchFailure` (`E047`), so `data` still carries the full publish response (`api_schema_hash`, `launch_status`, `launch_superseded`, `downstream_launches`, and so on) alongside the coded error.
