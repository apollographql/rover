# Spec: `rover dev` credential and license behavior

- **PRD:** [prd.md](./prd.md)
- **Jira:** [ROVER-428](https://apollographql.atlassian.net/browse/ROVER-428)
- **Status:** Draft
- **Owner:** Isaac Good

## 1. Purpose and scope

This spec defines the required behavior of `rover dev` with respect to GraphOS credentials and the offline enterprise license, per PRD goals 1 through 5. It describes what the system must do and the exact conditions and output involved, not how it's implemented. Implementation approach belongs in a separate design/implementation plan.

## 2. Terminology

- **Credentials**: an API key and graph ref usable by the router, supplied via `--graph-ref`, or via `APOLLO_KEY`/`APOLLO_GRAPH_REF` (environment or configured profile). `--graph-ref` additionally has a separate role of sourcing subgraph schemas from a published Studio variant for composition; that role is independent of the credential/license behavior this spec governs.
- **Offline license**: a license file supplied via `--license <path>`, letting the router validate its own entitlement without contacting GraphOS.
- **Enterprise features**: GraphOS Router Enterprise features and the `@connect` directive, both of which require either credentials or an offline license to function.
- **Local-only development**: running `rover dev` against local and/or directly reachable subgraph schemas, with no dependency on GraphOS Studio.

## 3. Functional requirements

### 3.1 Local-only development

- **FR1**: `rover dev` must be able to compose subgraphs and start a local router session using only local or directly reachable subgraph schemas, with no credentials and no offline license present. (Preserves current behavior; see PRD Goal 5.)
- **FR2**: In that mode, Enterprise features must be unavailable, consistent with current behavior.

### 3.2 Offline enterprise license

- **FR3**: `rover dev` must accept an optional `--license <path>` argument identifying an offline enterprise license file.
- **FR4**: When `--license <path>` is provided, the router session must start with that license applied, enabling Enterprise features without requiring credentials and without any network call to GraphOS for entitlement validation.
- **FR5**: `--license` must work independently of credentials: none of `--graph-ref`, `APOLLO_KEY`, or `APOLLO_GRAPH_REF` are required when a valid offline license is supplied.
- **FR6**: Credential resolution and forwarding to the router must not be conditioned on whether `--license` is also supplied. `rover dev` resolves and forwards credentials the same way regardless of `--license`'s presence. When both a license and credentials are present, precedence between them for Enterprise-feature entitlement is the router's own responsibility, not `rover dev`'s: the offline license takes priority, so no network call to GraphOS for entitlement validation occurs, without `rover dev` needing to suppress credential handling to achieve that.
- **FR7**: `rover dev` does not itself validate the license file's existence or contents ahead of time. If the supplied path is missing or the license is invalid, the resulting failure is whatever the router process reports, surfaced to the user the same way other router startup failures are today.

### 3.3 Startup notice for the fully unconfigured path

- **FR8**: When `rover dev` starts with no usable credentials (§2, an API key and a graph ref, from any source, that the router can actually use) and no `--license`, it must print a single, non-error, informational message at startup. The message must state that it's running without GraphOS credentials, that Enterprise features are disabled, and how to enable them via each of the three independent paths.

  Required text:
  > Running without GraphOS credentials. GraphOS Router Enterprise features and @connect are disabled. Pass --graph-ref, set APOLLO_KEY/APOLLO_GRAPH_REF, or pass --license to enable them.

- **FR9**: This message must not be printed when a credential- or license-related warning is already being printed for a partially configured or broken state (for example, a graph ref is set but the resolvable key isn't a graph key). Exactly one class of message fires for this concern per session: never both, never neither.
- **FR10**: The message must be printed at most once per `rover dev` session, not once per recomposition or hot-reload cycle.

### 3.4 Documentation

- **FR11**: `rover dev`'s documentation must state, prominently and before any credential-specific instructions, that local-only development works with zero GraphOS credentials.
- **FR12**: The documentation must enumerate all three independent ways to unlock Enterprise features (`--graph-ref`, `APOLLO_KEY`/`APOLLO_GRAPH_REF`, `--license`) and state plainly that none of them are required for local-only development.
- **FR13**: No flag's documented behavior, in `--help` or published docs, may describe functionality that isn't actually implemented. (Closes the `--license` dead-flag gap described in the PRD.)

## 4. Acceptance criteria

**Fully offline development**
Given no credentials and no `--license`, when `rover dev` starts against a local subgraph schema, then composition succeeds, a local router starts, and the FR8 notice is printed exactly once.

**Offline enterprise license**
Given a valid offline license file and no credentials, when `rover dev --license <path>` starts, then Enterprise features are enabled, no network call is made to GraphOS for entitlement, and the FR8 notice is not printed.

**License and credentials both supplied**
Given a valid offline license file and valid `--graph-ref`/`APOLLO_KEY`, when `rover dev --license <path>` starts, then credentials are resolved and forwarded to the router exactly as they would be without `--license`, Enterprise features are enabled via the license with no network call to GraphOS for entitlement, and the FR8 notice is not printed.

**Valid credentials**
Given `--graph-ref` and a valid graph-scoped key, when `rover dev` starts, then behavior is unchanged from today: Enterprise features are available, and the FR8 notice is not printed.

**Broken or partial credentials**
Given `--graph-ref` is set but the resolvable key isn't a graph key (or GraphOS is unreachable), when `rover dev` starts, then the existing warning behavior is preserved, and the FR8 notice is not also printed.

## 5. Non-goals

- Rover does not implement GraphOS Router's own license or entitlement validation logic; it only passes the option through.
- No new offline license distribution mechanism or format is introduced; this reuses the router's existing one.
- API-key scoping (the decoy-graph workaround) is out of scope, addressed separately by upcoming OAuth credential support.
- Behavior for users already supplying `--graph-ref` / `APOLLO_KEY` / `APOLLO_GRAPH_REF` today is unchanged, including when `--license` is also supplied (see FR6).
