---
category: feat
breaking: false
authors: [dotdat]
---

Profile-stored OAuth endpoint settings now take effect

`rover config set` already accepted `APOLLO_OAUTH_AUTHORIZATION_URL`, `APOLLO_OAUTH_TOKEN_URL`, `APOLLO_OAUTH_DEVICE_AUTHORIZATION_URL`, `APOLLO_OAUTH_REVOCATION_URL`, `APOLLO_OAUTH_WHOAMI_URL`, and `APOLLO_OAUTH_CLIENT_ID`, but nothing read the stored value - the `--oauth-*` flags always resolved to their built-in default first. They now go through the same profile tier as every other setting (flag > environment variable > profile > built-in default), appear in `rover config show`, and print the one-line override notice when a profile redirects an endpoint - only for the endpoints the running `rover auth` subcommand uses. An invalid stored value fails the command with `error.code` `E054`.
