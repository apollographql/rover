---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

Fail immediately on errors that a retry can't fix

A rejected API key, a permissions failure, or a bad endpoint URL used to be retried repeatedly for the full `--client-timeout` (30 seconds by default) before Rover said anything, so a mistyped key or URL took the better part of a minute to report. These now fail as soon as the answer comes back. Rate limits and server errors are still retried, since those do clear up on their own.
