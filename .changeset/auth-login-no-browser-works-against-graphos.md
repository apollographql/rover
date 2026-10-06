---
category: fix
breaking: false
authors: [dotdat]
---

`rover auth login --no-browser` works against GraphOS

The device-code login failed before it started, with "failed to request a device code: Failed to request a device code". Three things were wrong:

- Rover asked for a device code at `https://auth.apollographql.com/oauth2/device/authorize`, which doesn't exist. The endpoint the server advertises is `https://auth.apollographql.com/oauth2/device_authorization`, now the default for `APOLLO_OAUTH_DEVICE_AUTHORIZATION_URL`.
- Rover's OAuth client was registered before the registration asked for the device-code grant, so the server refused it with `unsupported_grant_type`. Rover now uses a client registered for both the browser and the device-code login: the default `APOLLO_OAUTH_CLIENT_ID` is now `UOsTLIgQb6eFevYcnQewoCDJZagFswCfESvr1hdIU8w`.
- The server's answer pointed at a Studio page that doesn't take the code. Rover now sends you to `/device` on the OAuth server, `https://auth.apollographql.com/device`, and no longer prints the server's "Or open this URL directly" link.

When the server refuses the request, the error now includes its reason, for example `Failed to request a device code: Server returned error response: unsupported_grant_type: Unsupported grant type`. A login made before this change keeps working until it expires. Logging out of one may print a warning that the old token couldn't be revoked; the credential is still removed.
