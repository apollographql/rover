import { buildSubgraphSchema } from '@apollo/subgraph';
import { readFileSync } from 'fs';
import { gql } from 'graphql-tag';
import { ApolloServer } from '@apollo/server';
import { startStandaloneServer } from '@apollo/server/standalone';

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

const { url } = await startStandaloneServer(server, { listen: { port: 4003 } });
console.log(`🚀 Pandas server ready at ${url}`);