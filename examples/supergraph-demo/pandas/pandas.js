const { buildSubgraphSchema } = require('@apollo/subgraph');
const { readFileSync } = require('fs')

const { ApolloServer, gql } = require('apollo-server');

const typeDefs = gql(readFileSync('./pandas.graphql', { encoding: 'utf-8' }).toString());

const pandas = [
  { name: 'Basi', favoriteFood: "bamboo leaves" },
  { name: 'Yun', favoriteFood: "apple" }
]

const resolvers = {
  Query: {
      allPandas: (_, args, context) => {
          return pandas;
      },
      panda: (_, args, context) => {
          return pandas.find(p => p.id == args.id);
      }
  },
}

const server = new ApolloServer({
  schema: buildSubgraphSchema({ typeDefs, resolvers })
});

server.listen({ port: 4003 }).then(({ url }) => {
    console.log(`🚀 Pandas server ready at ${url}`);
});
