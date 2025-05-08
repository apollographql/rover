import { buildSubgraphSchema } from "@apollo/subgraph";
import { ApolloServer } from "@apollo/server";
import { readFileSync } from "fs";
import gql from "graphql-tag";
import resolvers from "../resolvers";

describe("Product Mutation", () => {
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

  it("creates a new product", async () => {
    const mutation = `
      mutation CreateProduct($input: CreateProductInput!) {
        createProduct(input: $input) {
          id
          name
          description
        }
      }
    `;
    const variables = {
      input: { name: "Test Product", description: "A test product" },
    };
    const res = await server.executeOperation({ query: mutation, variables });
    expect(res.body.kind).toBe("single");
    const data = (res.body as any).singleResult.data.createProduct;
    expect(data.name).toBe("Test Product");
    expect(data.description).toBe("A test product");
    expect(typeof data.id).toBe("string");
  });

  it("rejects empty product name", async () => {
    const mutation = `
      mutation CreateProduct($input: CreateProductInput!) {
        createProduct(input: $input) {
          id
        }
      }
    `;
    const variables = { input: { name: "", description: "desc" } };
    const res = await server.executeOperation({ query: mutation, variables });
    expect(res.body.kind).toBe("single");
    expect((res.body as any).singleResult.errors[0].message).toMatch(/name is required/i);
  });

  it("rejects duplicate product name", async () => {
    const mutation = `
      mutation CreateProduct($input: CreateProductInput!) {
        createProduct(input: $input) {
          id
        }
      }
    `;
    const variables = { input: { name: "Test Product", description: "desc" } };
    const res = await server.executeOperation({ query: mutation, variables });
    expect(res.body.kind).toBe("single");
    expect((res.body as any).singleResult.errors[0].message).toMatch(/already exists/i);
  });
});
