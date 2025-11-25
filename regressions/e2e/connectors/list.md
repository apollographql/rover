# Lists the Connectors in a GraphQL schema file

## List_Connectors on fields

```console
$ rover connector --elv2-license accept list --schema fixtures/body.graphql
merging supergraph schema files
downloading the 'supergraph' plugin from https://rover.apollo.dev/tar/supergraph/x86_64-unknown-linux-gnu/latest-2
the 'supergraph' plugin was successfully installed to /home/runner/.rover/bin/supergraph-v2.12.1
{
  "connectors": [
    {
      "id": "query_helloWorld"
    }
  ]
}

```

## List_Connectors on types

```console
$ rover connector --elv2-license accept list --schema fixtures/single_entity.graphql
merging supergraph schema files
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
$ rover connector --elv2-license accept list --schema fixtures/multiple_connectors.graphql
merging supergraph schema files
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
$ rover connector --elv2-license accept list --schema fixtures/schema.graphql
merging supergraph schema files
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
$ rover connector --elv2-license accept list --schema fixtures/missing_type.graphql
merging supergraph schema files
{
  "connectors": []
}

```

## Errors

### No schema provided

```console
$ rover connector --elv2-license accept list
merging supergraph schema files
? 2
error: A schema path must be provided either via --schema or a `supergraph.yaml` containing a single subgraph

```