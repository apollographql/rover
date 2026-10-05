---
category: fix
breaking: false
authors: [dotdat]
---

`rover auth grants revoke --all` lists every pair instead of failing before it revokes anything

The sweep lists every client-credential pair in the organization before revoking, and asked the Platform API for all of them in one page size that overflowed to `-1`, so the listing was refused with "'first' must be positive, but -1 was given" and nothing was revoked. Each page now asks for at most 50 pairs, and the listing pages through the rest. `rover api-key list` with a `--limit` too large for a GraphQL `Int` is fixed the same way.
