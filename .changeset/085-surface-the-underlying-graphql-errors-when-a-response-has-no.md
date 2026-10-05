---
category: fix
breaking: false
authors: [sirdodger]
---

Surface the underlying GraphQL errors when a response has no `data` field

Requests that returned GraphQL errors alongside a null/missing `data` field previously showed a generic message instead of the actual error text. `GraphQLServiceError::NoData` now maps to the underlying errors (joined by newline) when there are any, falling back to the generic "No data field provided" message only when the response truly carried no errors either.
