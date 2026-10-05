---
category: fix
breaking: false
authors: [SharkBaitDLS]
---

`rover supergraph compose` no longer prints a blank line to stderr when composition raises no hints

Composition hints were rendered with a trailing newline and then printed with another, and an empty hint list still printed the empty string — so a run with nothing to say emitted a bare blank line to stderr, and a run that did raise hints ended them with a doubled newline. Hints now print without the extra newline, and produce no stderr output at all when there are none. stdout is unchanged, so a redirected supergraph schema is unaffected.
