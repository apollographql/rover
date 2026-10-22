# PRD: Rover Request Tracing

- **Component:** Rover CLI (Platform API request tracing)

This PRD lets Platform API failures seen by Rover be root-caused end to end. It is independent of Rover's anonymous usage telemetry (see `specs/rover-telemetry/prd.md`): it addresses the reliability of individual requests, not aggregate behavior, and it introduces no user or account identity.

## 1. Problem statement

Customers have reported a variety of errors coming back through Rover's error chain when running `rover supergraph compose`. Retries were added as a band-aid, but the underlying problems appear to originate in the Platform API itself. Root-causing them requires request-level data Rover does not currently send, so a failing request can't be followed from the client into the services that handled it.

## 2. Background: how this works today

- **Request tracing is absent.** Rover sends no W3C Trace Context headers on Platform API requests.
- **Retries are invisible server-side.** Rover retries some failing requests, but the platform has no way to tell how many attempts a request took or where it failed.
- **Rover telemetry is separate.** Anonymous usage telemetry goes to its own endpoint and is not correlated with Platform API requests.

## 3. Shared definitions

- **Trace Context headers.** The `traceparent` and `tracestate` headers defined by the W3C Trace Context standard.
- **Platform API gateway.** The entry point that receives Rover's Platform API requests and forwards them to internal services.

## 4. Goals

1. **Trace context on Platform API requests.** Rover sends `traceparent` and `tracestate` on requests to the Platform API, so a failure, or a set of failures, can be followed across every service involved in the request.
2. **Anonymous request-level data.** `tracestate` carries request-level data such as the number of retries attempted for that request, giving visibility into how often and where requests fail. It never carries anything user-identifiable.
3. **Platform-side support.** The Platform API gateway parses both headers, uses `traceparent` as the trace ID for subsequent logs and traces, forwards the data to its observability backends, and propagates the headers to internal services. This will allow us to observe rover requests as a top-level span in the Datadog APM trace viewer.

## 5. Key decisions and rationale

- **A standard for trace identity.** W3C Trace Context's `traceparent` is understood by observability tooling on the platform side, so the gateway doesn't need Rover-specific parsing to correlate a request's trace. The retry count is Rover-defined data and does need an encoding agreed with the gateway team.
- **Anonymous by construction.** `tracestate` carries request-level facts only, which keeps tracing outside the anonymity decision in the telemetry PRD.
- **Ownership is split.** The Rover team owns the Rover-side changes. The Platform API team owns gateway parsing, forwarding to observability backends, and propagation to internal services. This work has no user-visible value until the gateway side lands.
- **Future extension, not committed.** If users separately opt in, the anonymous usage data Rover already collects could ride alongside this channel for a more comprehensive field view. That is not part of this PRD.

## 6. Non-goals

- Account, org, or user identifiers in either header.
- Gateway-side implementation; this PRD records the dependency but the Platform API team owns the work.
- Tracing requests to destinations other than the Platform API.

## 7. Open questions (resolve in the spec)

- **Owning team and sequencing** for the gateway work, and whether Rover ships headers before the gateway consumes them.
- **Header scope.** Whether every Platform API request carries them or only selected operations.
- **Retry count transport.** Whether the retry count rides in a Rover-defined `tracestate` entry, which needs a key and encoding agreed with the gateway team and may be truncated by intermediaries, or in W3C `baggage`, the standard header for request-level data.
- **Privacy documentation.** Whether and how the headers are disclosed, given they are not part of the telemetry event.

## 8. Success metrics

- **Propagation.** A request from Rover to the Platform API carries both headers, and the gateway's logs for that request carry the same trace ID.
- **Retry visibility.** For a request that Rover retries, the retry count is visible on the platform side for the final attempt.
- **Anonymity.** No header value contains an identifier tied to a person, account, or organization.

---

## Rollout

- **Rover-side headers.** Its own spec and slice, sequenced with the Platform API team's gateway work.
- Each shipped slice is a user-visible change and gets a changelog entry.
