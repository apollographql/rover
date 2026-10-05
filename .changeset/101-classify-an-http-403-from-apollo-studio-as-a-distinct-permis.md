---
category: maint
breaking: false
authors: [dotdat]
---

Classify an HTTP 403 from Apollo Studio as a distinct permission-denied error

`rover-studio` gains a `PermissionDeniedLayer`, mirroring the existing `RejectedCredentialLayer` for the previously-unclassified case where a credential authenticates fine but isn't permitted to do something. Not yet wired into any command's service stack - foundation for rover-431's client-credential pair management.
