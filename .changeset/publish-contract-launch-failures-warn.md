---
category: fix
breaking: false
authors: [dotdat]
---

A failed contract-variant launch fails `rover graph publish` and `rover subgraph publish` only with `--include-contract-checks`

Both commands wait for the launches a publish triggers, and exited non-zero with E047 when any downstream contract-variant launch failed, though the schema was published and 0.x didn't fail on it. By default, a failed contract-variant launch is now a warning on stderr ("Warning: The publish succeeded, but a downstream contract launch failed: mobile. Pass --include-contract-checks to make this fail the command."), the command exits zero, and `--format json` reports no `error` while `data.downstream_launches` still shows the launch as `FAILED`. Pass `--include-contract-checks`, which already makes a blocking contract variant's failed check fail `publish --check`, to fail with E047 as before. A failed launch of the published variant itself still fails with E047 either way.
