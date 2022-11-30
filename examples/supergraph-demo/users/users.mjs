import { buildSubgraphSchema } from '@apollo/subgraph';
import { readFileSync } from 'fs';
import { gql } from 'graphql-tag';
import { ApolloServer } from '@apollo/server';
import { startStandaloneServer } from '@apollo/server/standalone';

const users = [
  { email: 'mara@acmecorp.com', name: "Mara", totalProductsCreated: 4 }
]

const typeDefs = gql(readFileSync('./users.graphql', { encoding: 'utf-8' }).toString());

const resolvers = {
  User: {
    __resolveReference: (reference) => {
      return users.find(u => u.email == reference.email);
    }
  }
}

const server = new ApolloServer({
  schema: buildSubgraphSchema({ typeDefs, resolvers })
});

const { url } = await startStandaloneServer(server, { listen: { port: 4001 } });
console.log(`🚀 Users server ready at ${url}`);