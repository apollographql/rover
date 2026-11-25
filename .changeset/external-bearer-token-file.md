---
category: feat
breaking: false
authors: [dotdat]
---

Authenticate Platform API requests with an application-managed token file

Use `--access-token-file` or `APOLLO_ACCESS_TOKEN_FILE` without a separate Rover login.
Rover checks and reads the file before every attempt, supports atomic token replacement,
and keeps refresh and token-file ownership with the supplying application.
In `rover dev`, the router continues to receive only its separate `APOLLO_KEY`.
