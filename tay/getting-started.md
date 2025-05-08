
👋 Hi there! This guide walks you through building a _subgraph_ with Apollo Server, TypeScript, and Apollo Federation.

- [Setup](#setup)
  - [Apollo Federation components](#apollo-federation-components)
  - [Components of a GraphQL subgraph server](#components-of-a-graphql-subgraph-server)
    - [The schema (`products.graphql`)](#the-schema-productsgraphql)
    - [Resolvers (`src/resolvers`)](#resolvers-srcresolvers)
    - [The server (`src/index.ts`)](#the-server-srcindexts)
- [Make your first request](#make-your-first-request)
  - [To the subgraph server (http://localhost:4001)](#to-the-subgraph-server-httplocalhost4001)
  - [To the supergraph (http://localhost:4000)](#to-the-supergraph-httplocalhost4000)
  - [Choosing a port to work with](#choosing-a-port-to-work-with)
- [Time to build your API](#time-to-build-your-api)
- [Debugging your schema](#debugging-your-schema)
  - [Design your schema with Apollo’s IDE extensions](#design-your-schema-with-apollos-ide-extensions)
  - [Check for errors each time you save](#check-for-errors-each-time-you-save)
- [Publishing your changes to GraphOS Studio](#publishing-your-changes-to-graphos-studio)
- [Security](#security)
- [Additional resources](#additional-resources)
  - [More on GraphQL server development](#more-on-graphql-server-development)
  - [More on federation](#more-on-federation)
  - [Deploying your supergraph](#deploying-your-supergraph)

# Setup

## Apollo Federation components

This project gets you started with building a federated architecture called a _supergraph_. This architecture lets different teams independently develop and deploy parts of the supergraph while maintaining a unified experience for clients.

A supergraph contains a _router_, and one or more _subgraphs_.

The router is the single access point for the supergraph. It receives incoming operations from clients and intelligently routes them across subgraphs before returning a unified response.

A subgraph is an individual GraphQL server that takes responsibility for a specific domain in the supergraph.

## Components of a GraphQL subgraph server

Before diving in, it's helpful to understand the structure and purpose of the files included in this template. This overview will help you navigate the codebase more effectively.

### The schema (`products.graphql`)

The schema describes what data is available, how it’s structured, and how it can be requested or modified. It’s written using GraphQL’s Schema Definition Language (SDL), which lets you define the shape and capabilities of an API in a clear, type-safe way that is also backend-agnostic.

### Resolvers (`src/resolvers`)

A resolver function populates the data for a particular field in the schema. Resolvers are defined in a resolvers map that follows the hierarchy of the schema.

You can find the resolvers for this project in `src/resolvers`. Each file corresponds to a type in your schema.

### The server (`src/index.ts`)

The server is in charge of making sure requests are valid, finding the right data, and sending it back to the requester.

**📓 Note:** This graph is using [Apollo Server](https://github.com/apollographql/apollo-server)—an open source server library that is quick and easy to set up, giving you a way to build a production-ready, self-documenting GraphQL API.


# Make your first request

## To the subgraph server (http://localhost:4001)

1. Open `products.graphql` and take a look at your starter schema.
2. In the terminal, run `npm ci`, then `npm run dev` to start the subgraph server.
3. In the browser, go to http://localhost:4001, where the subgraph server is running. You'll have access to Apollo Sandbox—a local, in-browser GraphQL playground, where you can run GraphQL operations and test your API as you design it.
4. In Sandbox, paste the following GraphQL query in the **Operation** section:

```
query GetProducts {
  products {
    id
    name
    description
  }
}
```

5. Click `► GetProducts` to run the request. You'll get a response back with data for the product's id, name, and description; exactly the properties you asked for in the query! 🎉

## To the supergraph (http://localhost:4000)

1. In a _new_ terminal window, run the `rover dev` command provided in the output of `rover init` under **Next steps**. The `dev` command starts a local development session with the router.
2. In the browser, go to http://localhost:4000, where `rover dev` is running. You'll have access to another Sandbox. Make sure you still have the subgraph server running from the previous section.
3. In Sandbox, paste the same GraphQL query in the **Operation** section:

```
query GetProducts {
  products {
    id
    name
    description
  }
}
```

4. Click `► GetProducts` to run the request. You'll get the same response back as before, but this time, the request was handled by the router.

## Choosing a port to work with

As you start building your schema, you can use Apollo Sandbox to send requests to your subgraph or supergraph.

When you want to test your subgraph server in isolation, use the Sandbox running at http://localhost:4001, started by the `npm run dev` command. This is recommended when you're just starting out, or when you want to focus on a specific subgraph server.

When you want to test the supergraph as a whole, use the Sandbox running at http://localhost:4000, started by the `rover dev` command. This is recommended when you have more than one subgraph in your supergraph.

📓 **Note:** If you are using `rover dev` and `localhost:4000`, you'll need to start the subgraph server _first_ by running `npm ci` and `npm run dev`—otherwise, you'll encounter errors when running requests in Sandbox.

# Time to build your API

You’re all set to start building.


First, make sure you’ve installed and configured your [IDE extension of choice](https://www.apollographql.com/docs/graphos/schema-design/ide-support) so you can rely on its autocompletion, schema information, and syntax highlighting features.

Then, follow the development cycle below:

1. Define the types and fields in the schema.
2. Write the resolver function(s) that provide the data for those types and fields.
3. Run operations and debug your API following the instructions in the section below.

📓 **Note:** The [GraphQL Code Generator](https://the-guild.dev/graphql/codegen) has been automatically set up and configured for you. It reads your GraphQL schema and generates TypeScript types to use across your server. This helps you keep your TypeScript types up to date as you make changes to your schema, allowing you to focus on development instead of manually updating type definitions.

Whenever you modify your schema, run `npm run codegen` to ensure your generated types are up to date as well.

ℹ️ **Tip:** If you run into any issues or difficulties, please reach out via the [Apollo Community](https://community.apollographql.com/c/graph-os/getting-started/35). Click **New Topic** to start a discussion–the Apollo team is here to help!

# Debugging your schema

The Apollo dev toolkit includes a few debugging tools to help you design and develop your graph. The journey looks a little something like this:

1. Design your schema with Apollo’s IDE extensions
2. Check for errors each time you save
3. Rinse and repeat until you're happy with your API!

## Design your schema with Apollo’s IDE extensions

Apollo’s IDE extensions are designed to help you catch and correct any issues related to schema design as early as possible. Lean on their instant feedback and autocomplete capabilities to help you create types, fields, and arguments.

## Check for errors each time you save

With `rover dev`, Rover starts watching your files for updates. Every time you make a change, Rover checks to see if the schema is valid. You can think of it as “hot-reloading” for your GraphQL schema. [More details about the dev command](https://www.apollographql.com/docs/rover/commands/dev).


# Publishing your changes to GraphOS Studio

When you publish a schema to GraphOS, it becomes part of your schema’s version history and is available for checks, composition, and collaboration. When you run `rover init`, GraphOS takes care of your first publish for you.

Once you’ve made changes to your schema files and are happy with the state of your API, or if you’d like to test the experience of publishing schema changes to GraphOS Studio, paste and run the following command in your terminal:

```
rover subgraph publish your-graph-id@main \ # Replace this with your `APOLLO_GRAPH_REF` value
  --schema "./products.graphql" \
  --name products-subgraph \
  --routing-url "https://my-running-subgraph.com/api" # If you don't have a running API yet,replace this with http://localhost:4000
```

📓 **Note:** For production-ready APIs, [integrating Rover into your CI/CD](https://www.apollographql.com/docs/rover/ci-cd) ensures schema validation, reduces the risk of breaking changes, and improves collaboration.

# Security

For a more secure and reliable API, Apollo recommends updating your CORS policy and introspection settings for production or any published/publicly accessible environments. You can do so by:

- Specifying which origins, HTTP methods, and headers are allowed to interact with your API
- Turning off GraphQL introspection to limit the exposure of your API schema

Making these updates helps safeguard your API against common vulnerabilities and unauthorized access. To learn more, [review Apollo’s documentation on Graph Security](https://www.apollographql.com/docs/graphos/platform/security/overview).

# Additional resources

## More on GraphQL server development

- [GraphQL basics](https://graphql.com/learn/what-is-graphql/)
- [How does a GraphQL server work?](https://graphql.com/learn/how-does-graphql-work/)
- [Introduction to Apollo Server](https://www.apollographql.com/docs/apollo-server)

## More on federation

- [Introduction to Apollo Federation](https://www.apollographql.com/docs/graphos/schema-design/federated-schemas/federation)
- [Tutorial: Federation with TypeScript & Apollo Server](https://www.apollographql.com/tutorials/intro-typescript)
- [More educational materials covering TypeScript and Federation](https://www.apollographql.com/tutorials/browse/?categories=federation&languages=TypeScript)

## Deploying your supergraph

- [Supergraph routing with GraphOS Router](https://www.apollographql.com/docs/graphos/routing/about-router)
- [Self-hosted Deployment](https://www.apollographql.com/docs/graphos/routing/self-hosted)
- [Router configuration](https://www.apollographql.com/docs/graphos/routing/configuration)
