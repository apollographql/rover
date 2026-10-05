---
category: fix
breaking: false
authors: [dotdat]
---

A response Rover can't parse now says where it failed

When a GraphOS response couldn't be parsed, Rover reported only `Deserialization error` (`E012`), leaving no way to tell a non-JSON reply (an outage or proxy page) from a response of the wrong shape. The message now names the HTTP status, whether the failure was a syntax error (not JSON at all) or a data error (JSON of the wrong shape), and the line and column. It still never quotes the response body, which may hold a secret.
