---
category: feat
breaking: false
authors: [dotdat]
---

Rover sends a `traceparent` header on every Platform API request

Every Platform API request, and every retry of it, carries a W3C Trace Context `traceparent` header. All requests in one run share a randomly generated trace ID, and each attempt has its own parent ID, so a set of failures from one command can be followed together. The ID is generated fresh for each run and isn't derived from your telemetry session, machine, or account.

Requests to other destinations don't carry it: introspection endpoints, the plugin registry, OAuth, and telemetry. Turning telemetry off doesn't stop Rover sending it.
