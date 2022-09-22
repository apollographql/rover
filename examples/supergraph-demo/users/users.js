const { buildSubgraphSchema } = require('@apollo/subgraph');
const { readFileSync } = require('fs')

const { ApolloServer, gql } = require('apollo-server');

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

server.listen({ port: 4001 }).then(({ url }) => {
    console.log(`🚀 Users subgraph ready at ${url}`);
});
