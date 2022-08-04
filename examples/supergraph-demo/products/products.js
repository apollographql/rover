const { buildSubgraphSchema } = require('@apollo/subgraph');
const { readFileSync } = require('fs')

const { ApolloServer, gql } = require('apollo-server');

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

server.listen({ port: 4002 }).then(({ url }) => {
    console.log(`🚀 Products server ready at ${url}`);
});
