# Spec: Rover request tracing behavior

- **PRD:** [prd.md](./prd.md)

## 1. Purpose and scope

This spec defines the required behavior of the trace headers Rover sends on Platform API requests, per PRD goals 1 and 2. It describes what Rover must do and the exact conditions involved, not how it's implemented. Implementation approach belongs in a separate design/implementation plan.

Gateway behavior (parsing the headers, forwarding to observability backends, and propagating to internal services) is PRD goal 3. The Platform API team owns it, and this spec doesn't define it. The header format below is the contract between the two sides.

## 2. Terminology

- **Platform API request**: an HTTP request Rover sends to the Platform API endpoint it's configured to use, whether that's the default or one set by configuration, including a registry-URL override that points at a proxy. Requests to the OAuth authorization service are not Platform API requests.
- **Logical request**: one operation Rover asks the Platform API to perform. Repeating it after a failure is a retry. Repeating a request to check on progress, as when polling for a result, is a new logical request each time, not a retry.
- **Attempt**: one send of a logical request over HTTP. A request that Rover retries has more than one attempt.
- **Retry**: a repeat of a logical request after an attempt failed because of a timeout, a connection failure, or a retryable response status.
- **Retry count**: the number of earlier attempts of the same logical request, across every retry mechanism Rover applies to it, before the current attempt. The first attempt has a retry count of 0.
- **Trace ID**: the 16-byte trace identifier of the W3C Trace Context `traceparent` header.
- **Parent ID**: the 8-byte parent identifier of the `traceparent` header.

## 3. Functional requirements

### 3.1 Scope

- **FR1** (Goal 1): Every attempt of every Platform API request must carry a `traceparent` header and a `tracestate` header.
- **FR2** (Goal 1): No request to any other destination may carry either header. That includes user-supplied endpoints such as introspection targets, the plugin registry, the OAuth authorization service, and the telemetry endpoint.
- **FR3** (Goal 1): Trace headers must be sent regardless of Rover's telemetry settings. `APOLLO_TELEMETRY_DISABLED`, `--telemetry-disabled`, and the profile and project telemetry settings must not suppress them.

### 3.2 Trace identity

- **FR4** (Goal 1): `traceparent` must follow the W3C Trace Context format: version `00`, a non-zero 16-byte trace ID, a non-zero 8-byte parent ID, and trace flags, each lowercase hexadecimal.
- **FR5** (Goal 1): Rover must generate one random trace ID per invocation. Every Platform API request in that invocation, and every attempt of each, must carry the same trace ID, so a set of failures can be followed together.
- **FR6** (Goal 1): Each attempt must carry a new random parent ID. Two attempts must not share a parent ID.
- **FR7** (Goal 1): The sampled flag in the trace flags must be set on every attempt.
- **FR8** (Goal 2): The trace ID and parent IDs must be generated from a cryptographically secure random source. They must not be computed from, or set equal to, the telemetry session identifier, the machine identifier, or any other identifier, hash, or stored value.

### 3.3 Retry count

- **FR9** (Goal 2): Each attempt's `tracestate` must contain exactly one `rover` entry whose value is `retries:` followed by the attempt's retry count in decimal, for example `rover=retries:2`.
- **FR10** (Goal 2): The first attempt of a logical request must carry `rover=retries:0`. Each retry must carry a count one higher than the attempt it follows. The count must include attempts made by every retry mechanism Rover applies to the request, on every request path, and must not reset or double count when mechanisms nest.
- **FR11** (Goal 2): The retry count must count only attempts of the same logical request. It must not carry over to the next request, and must not accumulate across requests in an invocation. Each request in a polling sequence is a new logical request, so its first attempt carries `rover=retries:0`.

### 3.4 Anonymity

- **FR12** (Goal 2): No header value may contain an identifier tied to a person, account, or organization. This governs the content of the headers only. Platform API requests are authenticated, so the server can associate any request, and therefore its trace, with the credential it carries. `tracestate` must carry only the entry in FR9, and `traceparent` must carry only the values in FR4 through FR7.
- **FR13** (Goal 2): Rover must not send a `baggage` header on Platform API requests.

### 3.5 Failure handling

- **FR14** (Goals 1, 2): A failure to generate or attach the trace headers must not fail, delay, or change the behavior of the request, and must print nothing to the user at the default log level. The request is sent without the headers, and the failure may be logged at debug level.
- **FR15** (Goals 1, 2): Adding the headers must not change the retry behavior: which responses are retried, how many attempts are made, and how long Rover waits.

### 3.6 Disclosure

- **FR16** (Goals 1, 2): The "Privacy and Data Collection" documentation must describe the trace headers: that Rover sends them on Platform API requests, what each carries, that their values hold no personally identifying information, that the requests are authenticated and so are associated with the credential on the server side, and that telemetry opt-outs don't suppress the headers. For each released version, the documentation must match the headers that version sends.

### 3.7 Visibility

- **FR17** (Goals 1, 2): At the debug log level, Rover must log the invocation's trace ID once, when the first Platform API request is sent. The log entry for each failed attempt must include the trace ID, the attempt's parent ID, and its retry count, so a user can give them to support.

## 4. Acceptance criteria

**Headers on a Platform API request**
Given a command that makes a Platform API request, when the request is sent, then it carries a valid `traceparent` and a `tracestate` of `rover=retries:0`.

**Same trace across requests**
Given an invocation that makes several Platform API requests, when each is sent, then every one carries the same trace ID.

**Distinct parent IDs**
Given a request that Rover retries, when each attempt is sent, then the attempts carry the same trace ID and different parent IDs.

**Retry count on retries**
Given a request that fails and is retried twice, when each attempt is sent, then the attempts carry `rover=retries:0`, `rover=retries:1`, and `rover=retries:2` in order.

**Retry count resets per request**
Given a request that was retried and a following request that wasn't, when the following request is sent, then it carries `rover=retries:0`.

**Polling is not retrying**
Given a command that polls the Platform API for a result, when each poll is sent, then its first attempt carries `rover=retries:0`, whatever happened on earlier polls.

**Every request path and nested retries**
Given a Platform API request sent through the legacy blocking client and one sent through the service-based client, when each is retried, then both carry the same headers by the same rules, and where retry mechanisms nest, the retry count equals the total number of earlier attempts.

**Other destinations**
Given a command that makes an introspection request to a user-supplied endpoint, a plugin download, an OAuth login or token request, or sends telemetry, when each request is sent, then none carries `traceparent` or `tracestate`.

**Debug logging**
Given a command run at the debug log level that makes a Platform API request, when the first request is sent, then the trace ID is logged once. Given an attempt that fails, then its log entry includes the trace ID, parent ID, and retry count.

**Telemetry opt-out**
Given `APOLLO_TELEMETRY_DISABLED` set to any value or `--telemetry-disabled` passed, when a command makes a Platform API request, then it still carries both headers.

**Randomness and independence**
Given a large sample of invocations, when their trace IDs are compared with each other and with the telemetry session identifier and machine identifier, then none is equal. The generator draws from a cryptographically secure random source and takes no identifier as input.

**Anonymity**
Given any Platform API request, when its headers are inspected, then `tracestate` contains only the `rover` entry, `traceparent` contains only the fields in FR4 through FR7, and there is no `baggage` header.

**Header failure**
Given a failure to generate the trace headers, when a command makes a Platform API request, then the request is sent without them, with the command's normal output and exit code.

**Unchanged retry behavior**
Given a request that is retried, when the headers are added, then the same responses are retried and the same number of attempts are made as without them.

**Documentation parity**
Given a released Rover version and its privacy documentation, when the documentation is compared with the headers the version sends, then they match.

## 5. Non-goals

- Account, org, or user identifiers in any header.
- Gateway-side behavior. Rover can ship before the gateway consumes the headers, because a header the gateway doesn't parse is ignored.
- Tracing requests to destinations other than the Platform API.
- Joining a trace supplied from outside Rover, such as one set in the environment.
- Carrying anonymous usage telemetry data on the trace headers.
