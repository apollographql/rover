---
category: feat
breaking: false
authors: [dotdat]
---

`rover auth grants revoke` revokes every grant a user holds in an organization

`rover auth grants revoke --org <ORGANIZATION_ID> --user <USER_ID> --all [--confirm]` revokes the user's grants under Rover's own OAuth client and under every client-credential pair in the organization. Before revoking anything, it lists every pair and asks for confirmation, naming each OAuth client. Pass `--confirm` to skip the prompt; without a terminal to ask on (or with `--format json`), it fails with a new error code (`E062`) instead of waiting. Every client is attempted even when one fails, and the outcome under each is reported individually; if any failed, the command exits non-zero with a new error code (`E063`) and names the clients to retry, which is safe to do because revoking where the user holds no grant succeeds. Access tokens the user already holds keep working until they expire, and the sweep doesn't stop them logging in again. `--format json` reports `organization_id`, `user_id`, `user_is_member`, `cancelled`, and a `clients` array of `client_id`, `name`, `kind`, `outcome`, and `error`.
