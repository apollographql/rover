# Error classification for telemetry outcomes

- **Spec:** [spec.md](./spec.md), FR16a and FR17

This table maps each of Rover's error codes to the telemetry outcome `user_error`, `internal_error`, or `connection_error`. It applies the principle in FR16a:

- **`user_error`**: the user can correct the cause. That covers bad input, configuration, authentication, permissions, schema or composition failures, check and lint failures, environment problems on the user's machine, and HTTP 4xx responses.
- **`connection_error`**: Rover couldn't reach a server, the connection dropped, or the request timed out, whichever side caused it.
- **`internal_error`**: Rover bugs, HTTP 5xx responses, and any other server error.
- **Ambiguous**: the code's own documentation lists several causes, and the code can't tell them apart. FR17 maps these to `internal_error`. They're marked so a reviewer can override them.

## Table

| Code | Cause | Outcome | Basis |
|---|---|---|---|
| E001 | GraphQL endpoint response isn't valid JSON | `internal_error` | Ambiguous: wrong endpoint or headers, or a registry fault |
| E002 | Invalid request header name | `user_error` | Clear |
| E003 | Invalid request header value | `user_error` | Clear |
| E004 | Request couldn't be executed (server unreachable, connection lost) | `connection_error` | Clear |
| E005 | Unexpected registry behavior | `internal_error` | Clear |
| E006 | Unexpected registry behavior | `internal_error` | Clear |
| E007 | `subgraph` command on a non-federated graph | `user_error` | Clear |
| E008 | Invalid variant | `user_error` | Clear |
| E009 | Subgraph name doesn't exist | `user_error` | Clear |
| E010 | Graph lookup or authentication failure | `user_error` | Clear |
| E011 | Introspection response can't be parsed | `internal_error` | Ambiguous: wrong endpoint, or a Rover parsing bug |
| E012 | Endpoint returned an HTTP 4xx or 5xx status | `user_error` for 4xx, `internal_error` for 5xx | Clear: decided by the response status (FR16a) |
| E013 | API key not recognized | `user_error` | Clear |
| E014 | API key in the wrong format | `user_error` | Clear |
| E015 | Update check found a malformed release version | `internal_error` | Clear |
| E016 | Can't create the configuration directory | `user_error` | Clear: permissions |
| E017 | Can't determine the default configuration directory | `user_error` | Clear: `APOLLO_CONFIG_HOME` is the fix |
| E018 | `APOLLO_CONFIG_HOME` points to a file | `user_error` | Clear |
| E019 | `rover config clear` found nothing to clear | `user_error` | Clear |
| E020 | No profile or API key found | `user_error` | Clear |
| E021 | Profile not found | `user_error` | Clear |
| E022 | Profile has nothing non-sensitive to load | `user_error` | Clear |
| E023 | Configuration path isn't valid UTF-8 | `user_error` | Clear |
| E024 | Profile holds invalid TOML | `user_error` | Clear: hand-edited file is the usual cause |
| E025 | Profile can't be serialized to TOML | `internal_error` | Clear |
| E026 | Trouble loading or saving a profile | `internal_error` | Ambiguous: a typo in the profile name, or a Rover fault |
| E027 | Composition failed | `user_error` | Clear |
| E028 | Couldn't connect to an HTTP endpoint | `connection_error` | Clear |
| E029 | Proposed subgraph schema couldn't be built | `user_error` | Clear |
| E030 | Operation check failed | `user_error` | Clear |
| E031 | HTTP request timed out | `connection_error` | Clear: timeouts are connection errors whichever side caused them |
| E032 | Async check started with invalid inputs | `user_error` | Clear |
| E033 | No permission for the operation | `user_error` | Clear |
| E034 | Billing plan limit reached | `user_error` | Clear |
| E035 | Corrupted API key in a profile (Windows, pre-v0.8.2) | `user_error` | Clear |
| E036 | Check failed on a task other than build, operations, or downstream | `user_error` | Clear |
| E037 | Downstream check failed | `user_error` | Clear |
| E038 | Supergraph configuration couldn't resolve all subgraph schemas | `user_error` | Clear |
| E039 | `contract` command on a non-contract variant | `user_error` | Clear |
| E040 | Contract configuration failed to publish | `user_error` | Clear: usually invalid inputs |
| E041 | New subgraph published without `--routing-url` | `user_error` | Clear |
| E042 | Schema has lint violations | `user_error` | Clear |
| E043 | Check step failed due to a schema change | `user_error` | Clear |
| E044 | Offline license not enabled for the organization | `user_error` | Clear |
| E045 | Operation failed after the maximum retries | `internal_error` | Clear: a temporary service issue |
| E046 | Can't read, write, or delete a credential in the keychain | `user_error` | Clear: locked or misconfigured keychain |
| E047 | Publish succeeded but a launch it triggered failed | `user_error` | Clear: usually the published schema |
| E048 | Can't resolve a plugin release | `internal_error` | Ambiguous: no matching version, or the plugin registry is unreachable |
| E049 | Plugin download failed | `connection_error` | Ambiguous: a connection failure or timeout, or an HTTP error from the registry. Treated as a connection error because the download is a network transfer |
| E050 | Plugin install failed | `internal_error` | Ambiguous: unwritable or full directory, or a corrupt archive |
| E051 | Requested plugin release was withdrawn | `user_error` | Clear |
| E052 | Plugin manifest can't be used | `user_error` | Clear |
| E053 | Platform API refused client-credential or grant management | `user_error` | Clear |
| E054 | Setting value fails its type's check | `user_error` | Clear |
| E055 | Profile has no credential | `user_error` | Clear |
| E056 | `rover api-key list` request failed | `internal_error` | Ambiguous: a timeout, a server error, or other failures |
| E057 | `rover api-key rotate` given an ID that isn't a client-credential pair | `user_error` | Clear |
| E058 | Plugin not installed and downloads disabled | `user_error` | Clear |
| E059 | `rover api-key rename` given a client-credential pair | `user_error` | Clear |
| E060 | Credential named in a project file's `settings:` | `user_error` | Clear |
| E061 | Same setting set twice in a project file | `user_error` | Clear |
| E062 | No terminal to confirm a grants revoke | `user_error` | Clear |
| E063 | Grants revoke partly failed | `internal_error` | Ambiguous: the Platform API's error per client |
| E064 | Federation 1 requested | `user_error` | Clear |
| E065 | Launch hadn't finished when Rover stopped waiting | `internal_error` | Ambiguous: a slow launch, or a platform delay; the wait is configurable |

## Counts

Each code is counted once. E012 is counted under `user_error` but also yields `internal_error` for 5xx responses.

- `user_error`: 48 codes
- `connection_error`: 4 codes (E004, E028, E031, E049)
- `internal_error`: 13 codes, of which 8 are ambiguous (E001, E011, E026, E048, E050, E056, E063, E065) and 5 are clear (E005, E006, E015, E025, E045)

## Decisions for review

1. **Remaining ambiguous codes.** E048, E050, E056, E063, and E065 involve the network but also have non-network causes, so they stay `internal_error`. They'd move to `connection_error` only if the code could tell a connection failure from the other causes.
2. **Errors with no code.** An error can carry no code, and FR17 maps those to `internal_error` for now. Classifying them is tracked as follow-up work, and this table is incomplete until that's done.
