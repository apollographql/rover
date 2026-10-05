---
category: fix
breaking: false
authors: [dotdat]
---

Report a rejected API key as `E013`/`E014` even when the registry responds `200 OK`

When the registry rejects a key by returning `HTTP 200` with `"data": null` and a body-level "Invalid credentials" error (rather than an `HTTP 401`/`406`), Rover used to report a generic, undifferentiated `error: No data field provided` instead of the usual `E013`/`E014`. The credential-rejection check only ever looked at a nested `extensions.response` field on each GraphQL error, never the error's own top-level `message`, which is where this particular response places the context. The check now looks at each error's top-level message, and runs regardless of whether `data` is present, `null`, or omitted. A related gap is fixed alongside it: `rover graph fetch` and `rover graph-artifact list-tags` now also check whether the rejected key was malformed (e.g. missing colons) before settling on `E013`, matching the check every other command already applied - so a key that was never validly shaped reports `E014` there too, not just `E013`.
