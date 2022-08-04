const { buildSubgraphSchema } = require('@apollo/subgraph');
const { readFileSync } = require('fs')

const { ApolloServer, gql } = require('apollo-server');

const typeDefs = gql(readFileSync('./schema.graphql').toString());

const resolvers = {
  Query: {
    mee() {
      return { id: "1", username: "@mara" }
    }
  }
};

const server = new ApolloServer({
  schema: buildSubgraphSchema({ typeDefs, resolvers })
});

server.listen({ port: 4003 }).then(({ url }) => {
    console.log(`🚀 Server ready at ${url}`);
});
