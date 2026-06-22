# Graph introspect fixtures

Test data for `crates/rover-client/src/operations/graph/introspect/`.

## SWAPI fixtures

Used by `introspection_json::tests::swapi_structural_parity_with_legacy_introspection_from_schema`.

| File | Role |
|------|------|
| `swapi.graphql` | SDL input for `sdl_to_introspection_json()` |
| `swapi-introspection.json` | Reference output: `{ "__schema": ... }` from graphql-js `introspectionFromSchema`, matching legacy `apollo schema:download` |
| `swapi.json` | Raw GraphQL introspection **response envelope** (`{ "data": { "__schema": ... } }`) from the server; used by `schema.rs` encode tests |

**Source endpoint:** https://swapi-graphql.netlify.app/graphql

### Regenerate `swapi.graphql` and `swapi-introspection.json`

These two files should be regenerated together. The JSON file is **not** the raw server response; it is introspection JSON rebuilt from the resolved schema, same as the old Apollo CLI (`JSON.stringify(introspectionFromSchema(schema))`).

**Option A — graphql-js (recommended; works on current Node):**

```bash
cd crates/rover-client/src/operations/graph/introspect/fixtures

node <<'EOF'
const { buildClientSchema, getIntrospectionQuery, printSchema, introspectionFromSchema } = require('graphql');
const https = require('https');
const fs = require('fs');

const endpoint = 'https://swapi-graphql.netlify.app/graphql';
const body = JSON.stringify({ query: getIntrospectionQuery() });

const url = new URL(endpoint);
const req = https.request({
  hostname: url.hostname,
  path: url.pathname,
  method: 'POST',
  headers: { 'Content-Type': 'application/json', 'Content-Length': body.length },
}, res => {
  let data = '';
  res.on('data', c => data += c);
  res.on('end', () => {
    const result = JSON.parse(data);
    if (result.errors) throw new Error(JSON.stringify(result.errors));
    const schema = buildClientSchema(result.data);
    fs.writeFileSync('swapi.graphql', printSchema(schema));
    fs.writeFileSync('swapi-introspection.json', JSON.stringify(introspectionFromSchema(schema), null, 2));
    console.log('Wrote swapi.graphql and swapi-introspection.json');
  });
});
req.on('error', e => { console.error(e); process.exit(1); });
req.write(body);
req.end();
EOF
```

Requires `npm install graphql` in a temp directory, or run from a folder that already has `graphql` installed.

**Option B — legacy Apollo CLI:**

```bash
cd crates/rover-client/src/operations/graph/introspect/fixtures

# Apollo CLI requires Node < 17; use nvm if needed:
# nvm install 16 && nvm use 16

npx apollo client:download-schema swapi.graphql \
  --endpoint=https://swapi-graphql.netlify.app/graphql

npx apollo client:download-schema swapi-introspection.json \
  --endpoint=https://swapi-graphql.netlify.app/graphql
```

Use the `.graphql` extension for SDL and `.json` for introspection JSON.

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

The parity test compares structurally, not byte-for-byte, because apollo-compiler `partial_execute` and graphql-js `introspectionFromSchema` may differ on meta-type fields, key ordering, and spec additions such as `specifiedBy` / `isRepeatable`.

## Other fixtures

| File | Used by |
|------|---------|
| `simple.json` | `runner.rs` HTTP mock tests |
| `interfaces.json` | `schema.rs` encode tests |
