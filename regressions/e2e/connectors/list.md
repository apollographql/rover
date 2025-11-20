# Lists the Connectors in a GraphQL schema file

## List_Connectors on fields

```console
$ rover connector --elv2-license accept list --schema regressions/fixtures/body.graphql
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
$ rover connector --elv2-license accept list --schema regressions/fixtures/single_entity.graphql
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
{
  "connectors": []
}

```

## Errors

### No schema provided

```console
$ rover connector --elv2-license accept list
? 2
error: the following required arguments were not provided:
  --schema <SCHEMA_PATH>

Usage: rover connector list --schema <SCHEMA_PATH>

For more information, try '--help'.

```