const { ApolloServer, gql } = require('apollo-server');
const { buildSubgraphSchema} = require("@apollo/subgraph")

// The GraphQL schema
const typeDefs = gql`
  type Query {
    "A simple type for getting started!"
    goodnight: String
  }
`;

// A map of functions which return data for the schema.
const resolvers = {
  Query: {
    goodnight: () => 'moon',
  },
};

const server = new ApolloServer({
 schema: buildSubgraphSchema({
   typeDefs,
   resolvers,
 })
});

server.listen({ port: 4002 }).then(({ url }) => {
  console.log(`🚀 Server ready at ${url}`);
});

