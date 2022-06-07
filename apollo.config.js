// this file is configuration for the VS Code extension
// docs: https://www.apollographql.com/docs/devtools/editor-plugins/
// install: https://marketplace.visualstudio.com/items?itemName=apollographql.vscode-apollo
// to configure, you need a `.env` file in this directory containing an `APOLLO_KEY=your_api_key`
// that can be created at https://studio-staging.apollographql.com/user-settings

module.exports = {
  client: {
    // Rover's introspection queries have their own local schemas that are not stored in Studio
    excludes: ["**/introspect_*.graphql"],

    // The extension usually looks in `src` for operations, but we keep them in a workspace crate
    includes: ["crates/rover-client/src/operations/**/*.graphql"],

    // The graph ref we rely on
    service: {
      name: "apollo-platform@current",
      url: "https://api.apollographql.com/api/graphql",
      localSchemaFile: "./crates/rover-client/.schema/schema.graphql",
    },
  },
};
