import { buildSubgraphSchema } from "@apollo/subgraph";
import { ApolloServer } from "@apollo/server";
import { readFileSync } from "fs";
import gql from "graphql-tag";
import resolvers from "../resolvers";

const server = new ApolloServer({
  schema: buildSubgraphSchema({
    typeDefs: gql(
      readFileSync("products.graphql", {
        encoding: "utf-8",
      })
    ),
    resolvers,
  }),
});

describe("Repository Template Functionality", () => {
  it("Executes Product Entity Resolver", async () => {
    //Arrange
    const query = `query ($representations: [_Any!]!) {
      _entities(representations: $representations) {
        ...on Product {
          name
          description
        }
      }
    }`;
    const variables = {
      representations: [{ __typename: "Product", id: "1" }],
    };
    const expected = {
      _entities: [{
        name: "Lunar Rover Wheels",
        description: "Designed for traversing the rugged terrain of the moon, these wheels provide unrivaled traction and durability. Made from a lightweight composite, they ensure your rover is agile in challenging conditions."
      }],
    };
    //Act
    const res = await server.executeOperation({
      query,
      variables,
    });
    //Assert
    expect(res.body.kind).toEqual("single");
    expect((res.body as any).singleResult.data).toEqual(expected);
  });
});
