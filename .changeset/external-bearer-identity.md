---
category: feat
breaking: false
authors: [dotdat]
---

Report application-supplied credentials in both identity commands

`rover auth whoami` and `rover config whoami` use the current token file for a
Platform API identity lookup in CI without a stored login or OAuth userinfo request.
Tokens are fully redacted unless `--insecure-unmask-key` is explicitly requested.
