---
category: fix
breaking: false
authors: [briangeorge]
---

`rover config whoami` no longer rejects a valid client-credentials identity

`APOLLO_CLIENT_ID`/`APOLLO_CLIENT_SECRET` authenticates as a service account, but `rover config whoami` only ever recognized `User`/`Graph` actor types and rejected everything else — including a fully valid, successfully-authenticated service account — with "The key provided is invalid. Rover only accepts personal and graph API keys". `Actor` (and the platform API response mapping that produces it) now has a `SERVICE_ACCOUNT` variant, so a service-account identity passes through cleanly, reporting `Key Type: Service Account` and its id under `User ID` (there's no dedicated field for it yet, and it's the closest existing fit).
