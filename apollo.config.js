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
    service: "engine@prod"
  },
  engine: {
    // This must be set so we pull SDL from studio-staging (the source of truth) instead of prod
    endpoint: 'https://graphql-staging.api.apollographql.com/api/graphql'
  }
};