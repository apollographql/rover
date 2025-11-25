# Lists the Connectors in a GraphQL schema file

## List_Connectors on fields

```console
$ rover connector --elv2-license accept list --schema regressions/fixtures/body.graphql
... merging supergraph schema files ...
... downloading the 'supergraph' plugin from https://rover.apollo.dev/tar/supergraph/x86_64-unknown-linux-gnu/latest-2 ...
... the 'supergraph' plugin was successfully installed to /home/runner/.rover/bin/supergraph-v2.12.1 ...
{
  "connectors": [
    {
      "id": "queery_helloWorld"
    }
  ]
}

```

## List_Connectors on types

```console
$ rover connector --elv2-license accept list --schema regressions/fixtures/single_entity.graphql
... merging supergraph schema files ...
... downloading the 'supergraph' plugin from https://rover.apollo.dev/tar/supergraph/x86_64-unknown-linux-gnu/latest-2 ...
... the 'supergraph' plugin was successfully installed to /home/runner/.rover/bin/supergraph-v2.12.1 ...
{
  "connectors": [
    {
      "id": "Product[0]"
    }
  ]
}

```

## Multiple Connectors

```console
$ rover connector --elv2-license accept list --schema regressions/fixtures/multiple_connectors.graphql
... merging supergraph schema files ...
... downloading the 'supergraph' plugin from https://rover.apollo.dev/tar/supergraph/x86_64-unknown-linux-gnu/latest-2 ...
... the 'supergraph' plugin was successfully installed to /home/runner/.rover/bin/supergraph-v2.12.1 ...
{
  "connectors": [
    {
      "id": "Query.helloWorld[0]"
    },
    {
      "id": "Query.helloWorld[1]"
    }
  ]
}

```

## List_Connectors with ID

```console
$ rover connector --elv2-license accept list --schema regressions/fixtures/schema.graphql
... merging supergraph schema files ...
... downloading the 'supergraph' plugin from https://rover.apollo.dev/tar/supergraph/x86_64-unknown-linux-gnu/latest-2 ...
... the 'supergraph' plugin was successfully installed to /home/runner/.rover/bin/supergraph-v2.12.1 ...
{
  "connectors": [
    {
      "id": "helloworld"
    }
  ]
}

```


## No Connectors Found

```console
$ rover connector --elv2-license accept list --schema regressions/fixtures/missing_type.graphql
... merging supergraph schema files ...
... downloading the 'supergraph' plugin from https://rover.apollo.dev/tar/supergraph/x86_64-unknown-linux-gnu/latest-2 ...
... the 'supergraph' plugin was successfully installed to /home/runner/.rover/bin/supergraph-v2.12.1 ...
{
  "connectors": []
}

```

## Errors

### No schema provided

```console
$ rover connector --elv2-license accept list
... merging supergraph schema files ...
... downloading the 'supergraph' plugin from https://rover.apollo.dev/tar/supergraph/x86_64-unknown-linux-gnu/latest-2 ...
... the 'supergraph' plugin was successfully installed to /home/runner/.rover/bin/supergraph-v2.12.1 ...
? 2
error: the following required arguments were not provided:
  --schema <SCHEMA_PATH>

Usage: rover connector list --schema <SCHEMA_PATH>

For more information, try '--help'.

```