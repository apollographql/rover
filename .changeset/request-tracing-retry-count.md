---
category: feat
breaking: false
authors: [dotdat]
---

Rover sends a `tracestate` header with the retry count on every Platform API request

Every Platform API request, and every retry of it, now carries a `tracestate` header alongside `traceparent`. Its one entry, `rover=retries:<n>`, says how many earlier attempts of the same request there were: `0` for the first attempt, `1` for the first retry, and so on. It carries nothing else, and no identifier for you, your machine, or your account.
