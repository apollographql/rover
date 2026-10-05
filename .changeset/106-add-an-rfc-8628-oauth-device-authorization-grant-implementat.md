---
category: maint
breaking: false
authors: [dotdat]
---

Add an RFC 8628 OAuth device authorization grant implementation to `rover-auth`

Adds `DeviceAuthorizationFlow` to `rover-auth`'s oauth2 module: requesting a device code, and polling the token endpoint until the user approves the request from another device. Not yet wired up to any command; a follow-up PR adds `rover auth login --no-browser`.
