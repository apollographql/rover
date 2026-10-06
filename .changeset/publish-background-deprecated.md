---
category: fix
breaking: false
authors: [dotdat]
---

`rover graph publish` and `rover subgraph publish` accept `--background` again, with a deprecation warning

Removing `--background` from both publish commands made it an argument error, so a 0.x script that passed it stopped working. It is accepted again, hidden from `--help`, and still has no effect: `publish --check` always waits for the check, because the check decides whether the publish happens. Passing it prints a warning that it has moved to `rover graph check --background` or `rover subgraph check --background`, and will be removed from the publish commands in a future version.
