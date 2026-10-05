---
category: maint
breaking: false
authors: [dotdat]
---

Add a non-retrying, timeout-bounded Studio GraphQL service constructor

`StudioClient` gains `studio_graphql_service_with_timeout`, a sibling to `studio_graphql_service` that swaps its ambient retry layer for a single per-attempt timeout - for a non-idempotent mutation where a retry could double the effect of a request the server already committed. Also adds `RoverClientError::PairPermissionDenied` (error code `E053`) and wires the new `PermissionDeniedLayer` into `studio_graphql_service_with_timeout` only, so a permission-denied response is classified for the operations that use it without changing behavior for `studio_graphql_service`'s existing callers. Not yet consumed by any command - foundation for rover-431's client-credential pair management.
