import { buildSubgraphSchema } from '@apollo/subgraph';
import { readFileSync } from 'fs';
import { gql } from 'graphql-tag';
import { ApolloServer } from '@apollo/server';
import { startStandaloneServer } from '@apollo/server/standalone';

const typeDefs = gql(readFileSync('./products.graphql', { encoding: 'utf-8' }).toString());

const products = [
  { id: 'payroll', sku: 'federation', package: '@apollo/federation', variation: "OSS" },
  { id: 'apollo-studio', sku: 'studio', package: '', variation: "platform" },
]

const resolvers = {
  Query: {
      allProducts: (_, args, context) => {
          return products;
      },
      product: (_, args, context) => {
          return products.find(p => p.id == args.id);
      }
  },
  ProductItf: {
      __resolveType(obj, context, info){
          return 'Product';
      },
  },
  Product: {
      variation: (reference) => {
          if (reference.variation) return { id: reference.variation };
          return { id: products.find(p => p.id == reference.id).variation }
      },
      dimensions: () => {
          return { size: "1", weight: 1 }
      },
      createdBy: (reference) => {
          return { email: 'mara@acmecorp.com', totalProductsCreated: 1337 }
      },
      __resolveReference: (reference) => {
          if (reference.id) return products.find(p => p.id == reference.id);
          else if (reference.sku && reference.package) return products.find(p => p.sku == reference.sku && p.package == reference.package);
          else return { id: 'rover', package: '@apollo/rover', ...reference };
      }
  }
}

const server = new ApolloServer({
  schema: buildSubgraphSchema({ typeDefs, resolvers })
});

const { url } = await startStandaloneServer(server, { listen: { port: 4002 } });
console.log(`🚀 Products server ready at ${url}`);
