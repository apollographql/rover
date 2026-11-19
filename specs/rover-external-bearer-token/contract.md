# Contract: Applications that supply a Platform API bearer token to Rover

- **PRD:** [prd.md](./prd.md)
- **Spec:** [spec.md](./spec.md)

## 1. Purpose

Rover consumes an Apollo OAuth access token that another application owns, and does not manage it. The [spec](./spec.md) defines what Rover does. This document defines what the integrating application must do for that to work safely. It describes observable obligations, not implementation.

Rover checks several of these conditions, as a second line of defense, and fails the affected operation when one is broken. It cannot check the rest. Section 3 lists what happens when each obligation is not met.

## 2. Application obligations

### 2.1 Account

- **IR1:** The token must represent the same account for the whole life of any Rover process, so the application must not replace it with a token for a different account while a Rover process it started or is aware of is still running. Rover does not record or compare the account a token represents, and does not treat a replacement token for a different account as an error. A long-running command, such as `rover dev`, therefore continues under whichever account the current token represents.

### 2.2 Location

- **IR2:** The application must give Rover the location of the token file with the `--access-token-file` option or the `APOLLO_ACCESS_TOKEN_FILE` environment variable, never the token itself. The location cannot come from a profile or project configuration.
- **IR3:** A relative location is resolved against the working directory the Rover process started in. The application should supply an absolute location.
- **IR4:** The application must not also supply `APOLLO_KEY` (except to give the router `rover dev` starts its own key), `APOLLO_CLIENT_ID`, or `APOLLO_CLIENT_SECRET` to a Rover process that has a token file. Rover fails any authenticated operation that finds both. An empty `APOLLO_KEY` is ignored.

### 2.3 The token file

- **IR5:** The location must name a regular file, not a symbolic link. Symbolic links in the directories above it are allowed.
- **IR6:** On a filesystem that enforces Unix file permissions, the file must be owned by the user running Rover and must not be readable or writable by anyone else.
- **IR7:** On such a filesystem, the directory that holds the file must be owned by the user running Rover or by the system administrator, and must not be writable by its group or by other users. A directory that anyone can write to, such as `/tmp`, is refused even when it has the sticky bit set.
- **IR8:** The file must contain exactly one token and nothing else. The token must be made only of the characters of an RFC 6750 `b64token`: one or more letters, digits, or `-`, `.`, `_`, `~`, `+`, `/`, followed by any number of `=`. It may end with one LF or one CRLF. It must not contain a `Bearer` prefix, an `Authorization` header, other lines, other whitespace, or a byte order mark.
- **IR9:** The file must not be larger than 16 KiB.
- **IR10:** The token may be a JWT, but Rover does not inspect it. It sends the token as it is, including an expired one, and the Platform API decides whether it is acceptable.

### 2.4 Replacing and removing the token

- **IR11:** The application must replace the token atomically: write the replacement to a new file that meets IR5 to IR9, in the same directory, then move it over the old file. It must not rewrite the file in place. Rover reads the file before every Platform API attempt, so the next attempt uses the replacement without restarting Rover.
- **IR12:** The application must refresh the token before it expires and publish the replacement in time for the next attempt. Rover never receives or stores a refresh token, never refreshes the token itself, and never revokes it.
- **IR13:** The application owns the file's cleanup. Rover never creates, modifies, or deletes it. If the application removes the file or makes it invalid while a Rover process is running, Rover's next attempt fails and does not fall back to another credential.
- **IR14:** The token must be one the Platform API accepts. This depends on the OAuth client that issued it.

## 3. When an obligation is not met

| If the application… | Rover… |
|---|---|
| Supplies a token for a different account partway through a run (IR1) | Uses the new token without an error. Later operations, and an identity lookup, can resolve to a different account. |
| Supplies both a token location and another credential input (IR4) | Warns at startup, then fails the first authenticated operation with the names of the conflicting inputs. |
| Supplies an empty location (IR2) | Fails the affected operation as a source failure. |
| Provides a symbolic link, a file or directory that fails the ownership or permission checks, or a file over 16 KiB (IR5 to IR7, IR9) | Does not send the attempt, and reports which check failed and how to fix it. |
| Provides content that is not a single valid token, or a byte order mark (IR8) | Does not send the attempt, and does not repeat the contents in its output. |
| Rewrites the file in place (IR11) | May read a truncated token. If it still looks like a token, Rover sends it, the Platform API rejects it, and Rover handles that as a credential rejection. |
| Lets the token expire, or publishes a replacement late (IR12) | Fails the operation on a credential rejection. After a 401 or 406 status, Rover reads the file once more and resends the request once if the token has changed. It never waits for a new token. |
| Removes the file or makes it unreadable (IR13) | Does not send the attempt, does not reuse the previous token, and does not start a login. |
| Supplies a token the Platform API does not accept (IR14) | Fails the operation as a credential rejection and tells the caller to renew the credential in the supplying application. |

## 4. Diagnosing failures

When Rover is asked for JSON output, each of these has its own stable `error.code`, so the application can tell them apart without parsing messages:

- a source failure, with distinct codes for an empty location, a missing or unreadable file, an oversize file, invalid contents, and a file refused by another check
- a credential rejection
- conflicting authentication inputs
- a permission denial

An application should treat a credential rejection as a prompt to renew the token, a source failure as a problem with the file it publishes, conflicting inputs as a problem with the environment it gives Rover, and a permission denial as a problem with the account's access.
