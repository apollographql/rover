---
category: fix
breaking: false
authors: [dotdat]
---

A failed `publish --check` is E043, and `--format json` reports the check result

When `rover graph publish --check` or `rover subgraph publish --check` stopped because the check failed, it exited with "Schema checks must pass before publishing. Fix the check failures above and try again." and no error code, and `--format json` reported `"data": null`. It now fails with E043, the code `rover graph check` and `rover subgraph check` use for a failed check, and `data` carries the check result in the same shape as the check commands' JSON, with `json_version` `"3"`. The message is unchanged.
