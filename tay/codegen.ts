import { CodegenConfig } from "@graphql-codegen/cli";

const config: CodegenConfig = {
  schema: "./*.graphql",
  generates: {
    "./src/__generated__/resolvers-types.ts": {
      config: {
        federation: true,
        useIndexSignature: true,
        contextType: '../types/DataSourceContext#DataSourceContext',
        
      },
      plugins: ["typescript","typescript-resolvers"]
    },
  },
};

export default config;
