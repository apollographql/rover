## Introduction

👋 Welcome to working with GraphOS\!

This starter project is designed to help you set up a new GraphQL API using [Apollo Connectors for REST](https://www.apollographql.com/docs/graphos/schema-design/connectors) and your existing REST APIs.

This project allows you to:

* Design your API’s with help from a starter template  
* Spin up a local environment to test your new GraphQL API
* Push your schema changes to [the GraphOS platform](https://www.apollographql.com/docs/graphos/get-started/concepts/graphos)

## Next steps (Recommended Workflow)

1. Inspect the \`schema.graphql\` file. This is a starter schema that uses Apollo’s Connectors syntax. You’ll find more comments and notes throughout that file to help you get a sense of how connectors work.  
2. Run \`rover dev\` to start a development session with a local instance of Apollo Router. This will give you access to Apollo's Explorer (a local, in-browser GraphQL playground), where you can run operations and test your schema as you design it.  
3. In the Explorer playground, run the pre-populated example query (it’s generated from \`schema.graphql\`). Tip: You can also build your own queries by selecting fields from the Documentation panel.  
4. Head back to \`schema.graphql\`. You can start by populating the \`baseURL\` with any REST API(s) of your choice. Then, follow the instructions included to add new fields using \`@connect\`.  
5. For more detailed information, visit the Apollo Docs to dive into the following guides:  
   * [Connectors syntax examples](https://www.apollographql.com/docs/graphos/schema-design/connectors#connector-example)  
   * [Making HTTP requests](https://www.apollographql.com/docs/graphos/schema-design/connectors/requests)  
   * [Common usage patterns for Apollo Connectors](https://www.apollographql.com/docs/graphos/schema-design/connectors/usage-patterns)  
6. Once you start making changes, head back to Explorer to test your schema. If you encounter any errors when running operations, we highly recommend checking out the [Debugging section](https://www.placeholderfordebugging.com) of this file.  
7. \[Placeholder for steps related to \`supergraph.yaml\` and \`router.yaml\`, depending on the end state of those files\]

## Debugging your schema  The Apollo suite includes a few debugging tools that are meant to help you design and develop your GraphQL API—each tackling a different checkpoint along the way with a shift-left mindset. The journey looks a little something like this:

1. Design your schema with the Apollo Language Server   
2. Check for errors each time you save (\`rover dev\` does this for you)  
3. Debug connectors in the local Explorer (“Connectors Debugger” under the Response dropdown).  
4. Rinse and repeat until you're happy with your API\!

### Apollo Language Server

Apollo’s IDE extensions are powered by Apollo's Language Server (LSP), which is designed to help you catch and correct any issues related to schema design as early as possible. Lean on its instant feedback and autocomplete capabilities to help you create types, fields, arguments, and connectors.

### Composition

When you start a local development session with a federated graph, Rover starts watching your files for updates. Every time you make a change, Rover tests the effect of those changes across your entire federated graph (also known as a supergraph) to make sure your services can be composed successfully.

### Connectors Debugging Tool

![][image1]  
Once you run \`rover dev\` and start your local development session, you can access the Connectors Debugger by selecting it from the “Response” drop-down on the right side of your screen. The debugger will provide detailed insights into network calls–including response bodies, errors, and connector-related syntax. You can also visit Apollo's docs to [learn more about troubleshooting Connectors](https://www.apollographql.com/docs/graphos/schema-design/connectors/troubleshooting#return-debug-info-in-graphql-responses).

## File overview

### supergraph.yaml

A federated GraphQL API is also known as a supergraph (i.e. a unified GraphQL API composed of different services—known as subgraphs—that are managed through a router). This file defines the configuration for your supergraph.

### router.yaml

This file is used to configure your Apollo Router, the runtime component responsible for handling requests to your API.


[image1]: ./image.png