# `schemas/demo`

When composed, these schemas make a supergraph. They were taken from the [federation quickstart](https://www.apollographql.com/docs/federation/quickstart/setup) on 07/25/2022.

## `rover supergraph compose`

Run composition with `rover supergraph compose --config ./schemas/demo/supergraph.yaml from the root of the Rover repository.

## `rover subgraph publish`

Publish the schemas to Apollo Studio by creating a graph in Apollo Studio, and then running `rover subgraph publish <GRAPH_REF>@<VARIANT> --name <SUBGRAPH_NAME> --schema ./examples/flyby/schemas/products.graphql` and `rover subgraph publish <GRAPH_REF@VARIANT> --name <SUBGRAPH_NAME> --schema ./users.graphql.

## `rover subgraph check`

Try making a change to one of the schemas and then run `rover subgraph check <GRAPH_REF>@<VARIANT> --name <SUBGRAPH_NAME> --schema ./examples/flyby/schemas/users.graphql`.

### Testing

This directory is used for integration testing. If you want to run the `npm scripts`, you will need cargo, npm, and a .env file with access to the `flyby-rover` graph in Apollo Studio. CircleCI stores this token in $FLYBY_APOLLO_KEY.
