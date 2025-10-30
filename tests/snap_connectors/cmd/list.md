# Lists the Connectors in a GraphQL schema file

## Connectors on fields

```console
$ rover connector list --schema tests/e2e_connectors/fixtures/body.graphql
{
  "connectors": [
    {
      "id": "query_helloWorld"
    }
  ]
}

```

## Errors

### No schema provided

```console
$ rover connector list
? 2
error: the following required arguments were not provided:
  --schema <SCHEMA_PATH>

Usage: rover connector list --schema <SCHEMA_PATH>

For more information, try '--help'.

```