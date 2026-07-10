# Graph introspect fixtures

Test data for `crates/rover-client/src/operations/graph/introspect/`.

## SWAPI fixtures

Used by `introspection_json::tests::swapi_structural_parity_with_legacy_introspection_from_schema`.

| File | Role |
|------|------|
| `swapi.graphql` | SDL input for `sdl_to_introspection_json()` |
| `swapi-introspection.json` | Reference output: server-sourced `{ "__schema": ... }`, the external baseline the parity test compares against |
| `swapi.json` | Raw GraphQL introspection **response envelope** (`{ "data": { "__schema": ... } }`) from the server; used by `schema.rs` encode tests |

**Source endpoint:** `https://swapi-graphql.netlify.app/graphql`

### Regenerate `swapi.graphql` and `swapi-introspection.json`

These are committed **static baselines** and rarely need refreshing. They only need to change if the upstream SWAPI schema changes meaningfully. No JavaScript toolchain is required to regenerate them.

`swapi-introspection.json` is the server-sourced introspection JSON envelope's inner `__schema` (`{ "__schema": ... }`). It is kept independent of our own `sdl_to_introspection_json()` on purpose, so the parity test compares our output against an external reference rather than against itself.

**Refresh `swapi.graphql` (SDL input) with Rover itself:**

```bash
cd crates/rover-client/src/operations/graph/introspect/fixtures

cargo rover graph introspect https://swapi-graphql.netlify.app/graphql > swapi.graphql
```

**Refresh `swapi-introspection.json` (reference) with `curl` + `jq`:**

Run the same introspection query used by the operation (`../introspect_query.graphql`) against the endpoint and keep only the `__schema` object, dropping the `{ "data": ... }` envelope:

```bash
cd crates/rover-client/src/operations/graph/introspect/fixtures

QUERY=$(jq -Rs . < ../introspect_query.graphql)
curl -s https://swapi-graphql.netlify.app/graphql \
  -H 'Content-Type: application/json' \
  -d "{\"query\": $QUERY}" \
  | jq '{ "__schema": .data.__schema }' > swapi-introspection.json
```

### Regenerate `swapi.json` (raw server response)

Used only by `schema.rs` tests. Save the HTTP response body from a standard introspection query:

```bash
curl -s https://swapi-graphql.netlify.app/graphql \
  -H 'Content-Type: application/json' \
  -d '{"query":"query { __schema { queryType { name } } }"}' \
  | jq . > swapi-partial.json
```

For the full fixture used in tests, run the same introspection query as `introspect_query.graphql` against the endpoint and persist the `{ "data": { "__schema": ... } }` envelope (or copy from a successful `rover graph introspect` network trace once legacy fallback issues are resolved).

### After regenerating

```bash
cargo test -p rover-client introspection_json
cargo test -p rover-client graph::introspect::schema
```

The parity test compares structurally, not byte-for-byte, because apollo-compiler `partial_execute` and the server's introspection response may differ on meta-type fields, key ordering, and spec additions such as `specifiedBy` / `isRepeatable`.

## Other fixtures

| File | Used by |
|------|---------|
| `simple.json` | `runner.rs` HTTP mock tests |
| `interfaces.json` | `schema.rs` encode tests |
