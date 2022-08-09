const { ApolloServer, gql } = require('apollo-server');
const { buildSubgraphSchema } = require("@apollo/subgraph")

// The GraphQL schema
const typeDefs = gql`
  type Query {
    "A simple type for getting started!"
    maybe: String
  }
`;

// A map of functions which return data for the schema.
const resolvers = {
  Query: {
    maybe: () => 'more',
  },
};

const server = new ApolloServer({
  schema: buildSubgraphSchema({
    typeDefs,
    resolvers,
  })
});

server.listen({
  port:
    4003
}).then(({ url }) => {
  console.log(`🚀 Server ready at ${url}`);
});

