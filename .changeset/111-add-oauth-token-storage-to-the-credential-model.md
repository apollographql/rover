---
category: maint
breaking: false
authors: [dotdat]
---

Add OAuth token storage to the credential model

A profile's stored credential can now be an OAuth access token (with an optional refresh token and expiry), alongside the existing Personal API Key, in the same OS-native secret store added above. Requests made with an OAuth credential now send `Authorization: Bearer <token>` instead of `x-api-key`. This is internal plumbing — see `rover auth login` below for the command that now writes one.
