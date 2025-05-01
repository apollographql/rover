👋 Hi there! The following guide walks you through integrating REST APIs into your graph using [Apollo Connectors](https://www.apollographql.com/docs/graphos/schema-design/connectors).

- [Setup](#setup)
  - [Part two: Check out how Connectors work](#part-two-check-out-how-connectors-work)
- [Time to build your API](#time-to-build-your-api)
- [Debugging your schema](#debugging-your-schema)
  - [Design your schema with Apollo’s IDE extensions](#design-your-schema-with-apollos-ide-extensions)
  - [Check for errors each time you save](#check-for-errors-each-time-you-save)
  - [Debug Connectors in Sandbox](#debug-connectors-in-sandbox)
- [Publishing changes to Apollo Studio](#publishing-changes-to-apollo-studio)
- [Security](#security)
- [Additional resources](#additional-resources)
  - [Deploying your GraphQL API](#deploying-your-graphql-api)
  - [More on graph development](#more-on-graph-development)
  - [More about Connectors](#more-about-connectors)

# Setup

1. Open `products.graphql` to take a look at your graph's starter schema. Ignore the comments labeled with a ✏️ for now, we’ll get to them later.
2. Run `rover dev --supergraph-config supergraph.yaml` to start a development session. This gives you access to Apollo Sandbox—a local, in-browser GraphQL playground, where you can run GraphQL operations and test your API as you design it.
3. In Sandbox, paste the following GraphQL query in the **Operation** section:

```
query GetProducts {
  products {
    id
    name
    description
  }
}
```

4. Click `► GetProducts` to run the request. You'll get a response back with data for the product's id, name, and description; exactly the properties you asked for in the query! 🎉

## Part two: Check out how Connectors work

1. Let's find out where this data is coming from. Click the arrow next to **Response** and select the **Connectors Debugger** option.
2. Now, click the most recent request to review its details. In the **Request overview** tab, press the **cURL** button to copy the underlying HTTP request made to the REST API. 
3. Run this request in your terminal and compare it with what’s been configured using the `@connect` directive in `products.graphql`. You'll notice that some properties in the terminal response match to the `selection` mapping in the schema. This is the key to how Connectors work!

Hooray! You ran a query, got some data back, and reviewed what Connectors are like under the hood! Feel free to experiment some more–try tweaking the query to see what data you can retrieve. 🚀

# Time to build your API
You’re all set to start building. You'll be working primarily with the `products.graphql` file.

First, make sure you’ve installed and configured [your IDE extension of choice](https://www.apollographql.com/docs/graphos/schema-design/ide-support) so you can rely on its autocompletion, schema information, and syntax highlighting features.

Then, follow the development cycle below:

1. [Add your REST API details using @source](https://www.apollographql.com/docs/graphos/schema-design/connectors/directives#source). 
2. Define the types and fields you want your GraphQL API to expose. Use the inline comments labeled with a ✏️ to follow along.
3. [Configure the Connector's request details](https://www.apollographql.com/docs/graphos/schema-design/connectors/requests).
4. [Configure the Connector's response mapping](https://www.apollographql.com/docs/graphos/schema-design/connectors/responses). You can use the [Connectors Mapping Playground](https://www.apollographql.com/connectors-mapping-playground) to help convert JSON responses to and from GraphQL types.
5. Run operations and debug your API following the instructions in the section below.

📓 **Note:** If you’re working with APIs that require headers, you’ll need to include them in `products.graphql` and add a router configuration file (`router.yaml`) to your project directory.

To learn more about headers and other advanced features like configuring environment variables, telemetry, and authentication, visit [Apollo’s docs on working with Router](https://community.apollographql.com/c/graph-os/getting-started/35).

ℹ️ If you run into any issues or difficulties, please reach out via the [Apollo Community here](https://community.apollographql.com/c/graph-os/getting-started/35) and click “New Topic”–the Apollo team is here to help!

# Debugging your schema

The Apollo dev toolkit includes a few debugging tools to help you design and develop your graph. The journey looks a little something like this:

- Design your schema with Apollo’s IDE extensions
- Check for errors each time you save
- Debug Connectors in Sandbox
- Rinse and repeat until you're happy with your API!

## Design your schema with Apollo’s IDE extensions
Apollo’s IDE extensions are designed to help you catch and correct any issues related to schema design as early as possible. Lean on their instant feedback and autocomplete capabilities to help you create types, fields, arguments, and Connectors.

## Check for errors each time you save
When you run `rover dev`, Rover starts watching your files for updates. Every time you make a change, Rover checks to see if the schema is valid. You can think of it as “hot-reloading” for your GraphQL schema. [More details about the dev command](https://www.apollographql.com/docs/rover/commands/dev).

## Debug Connectors in Sandbox

![A screenshot of the Connectors debugger in Apollo Sandbox](connectors_debugger.png)

In Apollo Sandbox, you can access the Connectors Debugger by selecting it from the **Response** drop-down on the right side of your screen. The debugger will provide detailed insights into network calls, including response bodies, errors, and connector-related syntax. You can also visit Apollo's docs to [learn more about troubleshooting Connectors](https://www.apollographql.com/docs/graphos/schema-design/connectors/troubleshooting#return-debug-info-in-graphql-responses).

# Publishing changes to Apollo Studio
When you publish a schema to GraphOS, it becomes part of your schema’s version history and is available for checks, composition, and collaboration. When you run `rover init`, GraphOS takes care of your first publish for you.

Once you’ve made changes to your schema files and are happy with the state of your API, or if you’d like to test the experience of publishing schema changes to GraphOS Studio, paste and run the following command in your terminal:

```
rover subgraph publish your-graph-id@main \ # Replace this with your `APOLLO_GRAPH_REF` value
  --schema "./products.graphql" \
  --name products-subgraph \
  --routing-url "https://my-running-subgraph.com/api" # If you don't have a running API yet, you can replace this with http://localhost:4000
```

📓 **Note:** For production-ready APIs, [integrating Rover into your CI/CD](https://www.apollographql.com/docs/rover/ci-cd) ensures schema validation, reduces the risk of breaking changes, and improves collaboration. 

# Security

For a more secure and reliable API, Apollo recommends updating your CORS policy and introspection settings for production or any published/publicly accessible environments. You can do so by:

- Specifying which origins, HTTP methods, and headers are allowed to interact with your API
- Turning off GraphQL introspection to limit the exposure of your API schema

Making these updates helps safeguard your API against common vulnerabilities and unauthorized access. To learn more, [review Apollo’s documentation on Graph Security](https://www.apollographql.com/docs/graphos/platform/security/overview).

# Additional resources

## Deploying your GraphQL API
- [Supergraph routing with GraphOS Router](https://www.apollographql.com/docs/graphos/routing/about-router)
- [Self-hosted Deployment](https://www.apollographql.com/docs/graphos/routing/self-hosted)
- [Router configuration](https://www.apollographql.com/docs/graphos/routing/configuration)

## More on graph development

- [Introduction to Apollo Federation](https://www.apollographql.com/docs/graphos/schema-design/federated-schemas/federation)
- [Schema Design with Apollo GraphOS](https://www.apollographql.com/docs/graphos/schema-design)
- [IDE support for schema development](https://www.apollographql.com/docs/graphos/schema-design/ide-support)

## More about Connectors

- [Tutorial: GraphQL meets REST with Apollo Connectors](https://www.apollographql.com/tutorials/connectors-intro-rest)
- [Connectors Community Repo](https://github.com/apollographql/connectors-community)