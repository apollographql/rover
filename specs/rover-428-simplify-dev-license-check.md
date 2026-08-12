# PRD: Simplify the credential/license requirements for `rover dev`

- **Status:** Ready for Review
- **Owner:** Isaac Good
- **Component:** Rover CLI (`rover dev`, local router/composition)

## 1. Problem statement

Customers report growing friction starting a local dev environment with `rover dev`: it increasingly feels like it *requires* an API key, a graph ref, and other GraphOS-specific information before a developer can compose and run subgraphs locally. Concretely, from a large customer's feedback (JPD-255):

1. **Surprise at required inputs.** Customers weren't aware that `rover dev` had come to expect API keys / graph refs, and it broke their onboarding flow when it changed, apparently without being clearly communicated.
2. **Workarounds to limit blast radius.** To avoid handing local developers an API key scoped to their production supergraph, customers created a separate "development" graph in GraphOS purely to issue a lower-privilege key for `rover dev`. This is busywork that exists only to satisfy `rover dev`'s perceived requirements, not because the developer's work touches GraphOS at all. This specific complaint is expected to be resolved separately by upcoming OAuth credential support, which will expand access without requiring a graph ref; it's called out here for context but is out of scope for this PRD (see [§4](#4-non-goals)).

The underlying tension: **composition already works with zero GraphOS credentials today**, and `rover dev` itself does not hard-require an API key or graph ref either. But the product doesn't make that clear: the docs read as if credentials are mandatory, and dead/regressed CLI surface ([§2.3](#23---license-is-a-confirmed-accidental-regression-not-a-deprecation)) reinforces the impression that this is a fully gated, credential-required flow.

This PRD is scoped to three concrete, independently shippable fixes, rather than a single "remove the license check" change. There isn't one check to remove: the friction comes from a documentation gap, a shipped-then-silently-regressed feature, and a missing feedback signal.

## 2. Background: how this works today

Verified against `src/command/dev/`, since actual behavior is more permissive than the reported customer experience suggests.

### 2.1 There is no hard gate on credentials in `rover dev` itself

- Composition (`CompositionPipeline`, `src/composition/`) never touches Studio or any credential. It only requires accepting the ELv2 license once per machine, to install the `supergraph` composition binary (`src/composition/supergraph/install.rs`): a legal acknowledgment, not an entitlement check, and it never makes a network call.
- Whether `rover dev` even attempts to reach Studio for anything is driven entirely by whether `--graph-ref` is passed (`src/command/dev/do_dev.rs:255-261`, `src/command/dev/router/run.rs:110-145`). If it's absent, `RunRouter<state::LoadRemoteConfig>::load_remote_config` never constructs a `RemoteRouterConfig`, and no Studio call is made.
- If no `--graph-ref` is passed, `rover dev` still tries to attach a local `APOLLO_KEY` to the router's environment from whatever default Houston profile exists (`RunRouter::auth_env`, `src/command/dev/router/run.rs:248-290`), but on failure it **only warns**, and only if a non-default profile was explicitly requested (`router/run.rs:278-283`). A fresh machine with no configured credentials and no flags gets a router that starts with no `APOLLO_KEY` at all. It happens silently, with no message either way.
- Every failure mode when a graph ref/API key situation is ambiguous or broken (`src/command/dev/router/config/remote.rs:17-90`) resolves to a `warnln!` and continuing, never a hard error, e.g.:
  - `"APOLLO_GRAPH_REF is set, but could not communicate with Studio. Router may fail to start if Enterprise features are enabled: {err}"`
  - `"APOLLO_GRAPH_REF is set, but the key provided is not a graph key. Enterprise features within the router will not function. ..."`

So today's real behavior is: *credentials are optional inputs that unlock GraphOS Router Enterprise features, `@connect`, and Studio-hosted subgraphs/variants; local composition and a local router session work without them.* That's a reasonable design. It just isn't legible.

### 2.2 The docs overstate the requirement

[`docs/source/commands/dev.mdx`](../docs/source/commands/dev.mdx) mixes signals:

- Line 54 has the correct framing: *"To use GraphOS Router features or the `@connect` directive in your schema, provide the `APOLLO_KEY` and `APOLLO_GRAPH_REF` environment variables."*
- But the "GraphOS Router features" section (lines 200-218) and the MCP section describe key + graph ref in a way that reads as a prerequisite for `rover dev` broadly, with no equally prominent "you don't need any of this to run subgraphs locally" statement up front.
- There's no single, early decision tree: *local-only (no creds) vs. GraphOS Router Enterprise features (creds) vs. Studio-hosted variant as source of truth (creds)*.

This is likely the dominant source of the "growing amount of information" complaint: the requirement is opt-in per feature, but nothing in the docs says so plainly.

### 2.3 `--license` is a confirmed accidental regression, not a deprecation

`SupergraphOpts.license: Option<Utf8PathBuf>` (`src/command/dev/mod.rs:101-105`, the `--license` flag for an [offline enterprise license](https://www.apollographql.com/docs/router/enterprise-features/#offline-enterprise-license) file) is still declared, appears in `--help`, and links to real router docs. But nothing in `do_dev.rs` or `router/run.rs` reads it today, so passing it silently does nothing.

Git history resolves the "was this ever real?" question:

1. `--license` was added in **#2078** (`rover dev: add --license flag`), explicitly to satisfy a customer feature request, [GitHub issue #1937](https://github.com/apollographql/rover/issues/1937), for offline enterprise license support in `rover dev`. At that point it was fully wired up through `src/command/dev/protocol/leader.rs` and `src/command/dev/router/runner.rs`.
2. **#2228** ("Initial scaffold for rover dev rewrite") moved that entire implementation (including the leader/runner code that consumed `--license`) into a `legacy/` module, while building the new `do_dev.rs`/`router/run.rs` implementation alongside it. The new implementation never re-wired `--license`.
3. **#2352** ("ROVER-244 Remove dead code and tidy up") deleted the `legacy/` module outright, including the last code that ever read the `--license` value.

The CLI flag, its docstring, and its docs link all survived three PRs after the feature behind them was deleted. This was an unintentional regression, not a signal that offline licensing was deprioritized on purpose.

## 3. Goals

1. **Make "no GraphOS credentials required" a documented, intentional, legible default** for `rover dev` with local subgraphs, not an accidental side effect of warn-and-continue error handling that only becomes visible when something's half-broken.
2. **Restore `--license`**, wired to the router's existing offline enterprise license mechanism, so fully offline Enterprise usage works as originally shipped and as still documented. This directly serves the "start offline, within reason" ask from JPD-255 and closes a multi-year-old silent regression.
3. **Communicate the actual decision tree up front** in docs and CLI messaging: what works with zero credentials, what specifically requires a key + graph ref, and why, so credential requirements read as opt-in features, not a gate.
4. **Give the credential-free path an explicit, one-time confirmation at startup** (e.g., `Running without GraphOS credentials. GraphOS Router Enterprise features and @connect are disabled. Pass --graph-ref or set APOLLO_KEY/APOLLO_GRAPH_REF to enable them.`), so a developer running fully locally sees confirmation that this is a supported, intentional mode, not silence they have to trust.
5. Preserve all existing capabilities: Studio-hosted subgraph introspection, published variant startup (`--graph-ref`), Enterprise router features, and MCP server integration must keep working exactly as they do today for users who supply credentials.

## 4. Non-goals

- Changing how GraphOS Router itself enforces Enterprise feature entitlements at runtime (that check lives in the router binary, not Rover).
- Inventing new offline/air-gapped license distribution tooling: restoring `--license` should reuse the router's existing offline enterprise license mechanism, not a new one.
- Solving the API-key-scoping workaround (customers provisioning a decoy "development" graph for a lower-privilege key) directly. This is expected to be addressed by upcoming OAuth credential support, which will expand access without requiring a graph ref. That work is tracked separately from this PRD.
- Removing the customer's ability to point `rover dev` at a real GraphOS graph via `--graph-ref` / `APOLLO_GRAPH_REF`.

## 5. Success metrics

- A developer with no GraphOS account can go from `git clone` to a running local supergraph via `rover dev` using only a local schema/URL, and sees an explicit, friendly confirmation that they're running in credential-free mode rather than ambiguous silence.
- `--help` output and `dev.mdx` contain no flag or sentence that promises behavior the code doesn't implement: specifically, `--license` works as documented again.
- A customer with a valid offline enterprise license file can run `rover dev --license <path>` and get Enterprise router features without any network call to Studio.
- Support/community reports of "why does `rover dev` need an API key" trend down after docs, messaging, and `--license` restoration ship (qualitative, tracked via support channels).

## 6. Rollout considerations

- Docs and CLI help-text changes (Goal 3) can ship independently and immediately reduce confusion without any behavior change. Sequence this first.
- The startup notice (Goal 4) is a small, additive CLI change; needs a decision on where it lives relative to the existing `warnln!` messages in `router/run.rs` and `router/config/remote.rs` so the two don't talk past each other (e.g., the new notice should only fire when there are truly zero credentials, not when credentials exist but are broken, since that case already has its own warnings).
- Restoring `--license` (Goal 2) is the largest piece of work here: it requires re-wiring the flag into `router/run.rs`/`router/install.rs` to pass the license file path through to the router binary invocation, and needs e2e coverage for the offline-router-start path (per `tests/README.md` guidance on heavier e2e coverage for cross-platform/shell-sensitive flows). It's also a user-facing behavior change and needs a `CHANGELOG.md` entry per repo convention.
- No breaking changes are anticipated for existing `--graph-ref` / `APOLLO_KEY` / `APOLLO_GRAPH_REF` users: this work is additive (clarify, restore, and add a signal), not a change to the credentialed path.
