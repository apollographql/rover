---
category: fix
breaking: false
authors: [dotdat]
---

Redact application-supplied credentials reflected in Platform API errors

Response headers and malformed-response diagnostics also keep credentials out
of debug output, without changing successful GraphQL response data.
