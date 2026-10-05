---
category: fix
breaking: false
authors: [dotdat]
---

Register the device-code grant type for `rover auth login --no-browser`'s OAuth client

Every OAuth client provisioned via `cargo xtask register-oauth-client` (for both staging and prod) was registered with only the `authorization_code` grant type, never `urn:ietf:params:oauth:grant-type:device_code` - so `rover auth login --no-browser` always failed at the very first step, with the server rejecting the device-authorization request as `unsupported_grant_type`
