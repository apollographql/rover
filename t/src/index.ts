import { readFileSync } from "fs";
import gql from "graphql-tag";
import { buildSubgraphSchema } from "@apollo/subgraph";
import { ApolloServer } from "@apollo/server";
import {
  startStandaloneServer,
} from "@apollo/server/standalone";
import resolvers from "./resolvers";

const port = "4001";
const subgraphName = "products";

async function main() {
  let typeDefs = gql(
    readFileSync("products.graphql", {
      encoding: "utf-8",
    })
  );
  const server = new ApolloServer({
    schema: buildSubgraphSchema({ typeDefs, resolvers }),
  });
  const { url } = await startStandaloneServer(server, {
    listen: { port: Number.parseInt(port) },
  });

  console.log(`🚀  Subgraph ${subgraphName} ready at ${url}`);
}

main();
