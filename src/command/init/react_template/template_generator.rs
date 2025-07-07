use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use crate::command::init::react_template::npm_client::{SafeNpmClient, DependencyVersions};

pub struct PureRustViteGenerator {
    target_dir: PathBuf,
    project_name: String,
    graph_ref: String,
    endpoint: String,
}

impl PureRustViteGenerator {
    pub fn new(target_dir: PathBuf, project_name: String, graph_ref: String, endpoint: String) -> Self {
        Self {
            target_dir,
            project_name,
            graph_ref,
            endpoint,
        }
    }

    pub async fn generate(&self) -> Result<()> {
        // Create directory structure
        self.create_directory_structure()?;
        
        // Get latest dependency versions (optional, with fallback)
        let npm_client = SafeNpmClient::new();
        let deps = npm_client.get_deps_with_fallback().await;

        // Generate all files
        self.create_package_json(&deps)?;
        self.create_typescript_config()?;
        self.create_vite_config()?;
        self.create_index_html()?;
        self.create_source_files()?;
        self.create_apollo_setup()?;
        self.create_env_files()?;
        self.create_gitignore()?;
        self.create_readme()?;
        self.create_eslint_config()?;

        Ok(())
    }

    fn create_directory_structure(&self) -> Result<()> {
        let dirs = vec![
            "src",
            "src/apollo",
            "src/apollo/queries",
            "src/apollo/mutations",
            "src/apollo/fragments",
            "src/components",
            "src/hooks",
            "src/pages",
            "src/types",
            "src/assets",
            "public",
        ];

        for dir in dirs {
            fs::create_dir_all(self.target_dir.join(dir))?;
        }

        Ok(())
    }

    fn create_package_json(&self, deps: &DependencyVersions) -> Result<()> {
        let package_json = serde_json::json!({
            "name": self.project_name,
            "private": true,
            "version": "0.0.0",
            "type": "module",
            "scripts": {
                "dev": "vite",
                "build": "tsc && vite build",
                "lint": "eslint . --ext ts,tsx --report-unused-disable-directives --max-warnings 0",
                "preview": "vite preview",
                "codegen": "rover graph introspect --output schema.graphql",
                "type-check": "tsc --noEmit"
            },
            "dependencies": {
                "react": deps.react,
                "react-dom": deps.react_dom,
                "@apollo/client": deps.apollo_client,
                "graphql": deps.graphql
            },
            "devDependencies": {
                "@types/react": deps.types_react,
                "@types/react-dom": deps.types_react_dom,
                "@typescript-eslint/eslint-plugin": deps.typescript_eslint_plugin,
                "@typescript-eslint/parser": deps.typescript_eslint_parser,
                "@vitejs/plugin-react": deps.vite_plugin_react,
                "eslint": deps.eslint,
                "eslint-plugin-react-hooks": deps.eslint_plugin_react_hooks,
                "eslint-plugin-react-refresh": deps.eslint_plugin_react_refresh,
                "typescript": deps.typescript,
                "vite": deps.vite
            }
        });

        let content = serde_json::to_string_pretty(&package_json)?;
        fs::write(self.target_dir.join("package.json"), content)?;

        Ok(())
    }

    fn create_typescript_config(&self) -> Result<()> {
        let tsconfig = serde_json::json!({
            "compilerOptions": {
                "target": "ES2020",
                "useDefineForClassFields": true,
                "lib": ["ES2020", "DOM", "DOM.Iterable"],
                "module": "ESNext",
                "skipLibCheck": true,
                "moduleResolution": "bundler",
                "allowImportingTsExtensions": true,
                "resolveJsonModule": true,
                "isolatedModules": true,
                "noEmit": true,
                "jsx": "react-jsx",
                "strict": true,
                "noUnusedLocals": true,
                "noUnusedParameters": true,
                "noFallthroughCasesInSwitch": true
            },
            "include": ["src"],
            "references": [{ "path": "./tsconfig.node.json" }]
        });

        fs::write(
            self.target_dir.join("tsconfig.json"),
            serde_json::to_string_pretty(&tsconfig)?
        )?;

        let tsconfig_node = serde_json::json!({
            "compilerOptions": {
                "composite": true,
                "skipLibCheck": true,
                "module": "ESNext",
                "moduleResolution": "bundler",
                "allowSyntheticDefaultImports": true,
                "strict": true
            },
            "include": ["vite.config.ts"]
        });

        fs::write(
            self.target_dir.join("tsconfig.node.json"),
            serde_json::to_string_pretty(&tsconfig_node)?
        )?;

        Ok(())
    }

    fn create_vite_config(&self) -> Result<()> {
        let content = r#"import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    open: true
  }
})
"#;

        fs::write(self.target_dir.join("vite.config.ts"), content)?;
        Ok(())
    }

    fn create_index_html(&self) -> Result<()> {
        let content = format!(
            r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>{}</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
"#,
            self.project_name
        );

        fs::write(self.target_dir.join("index.html"), content)?;
        Ok(())
    }

    fn create_source_files(&self) -> Result<()> {
        // main.tsx
        let main_content = r#"import React from 'react'
import ReactDOM from 'react-dom/client'
import { ApolloProvider } from '@apollo/client'
import App from './App'
import { client } from './apollo/client'
import './index.css'

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ApolloProvider client={client}>
      <App />
    </ApolloProvider>
  </React.StrictMode>,
)
"#;
        fs::write(self.target_dir.join("src/main.tsx"), main_content)?;

        // App.tsx
        let app_content = r#"import { useState } from 'react'
import { useQuery } from '@apollo/client'
import { EXAMPLE_QUERY } from './apollo/queries/example'
import reactLogo from './assets/react.svg'
import apolloLogo from './assets/apollo.svg'
import viteLogo from '/vite.svg'
import './App.css'

function App() {
  const [count, setCount] = useState(0)
  const { loading, error, data } = useQuery(EXAMPLE_QUERY, {
    errorPolicy: 'all',
  })

  return (
    <div className="App">
      <div>
        <a href="https://vitejs.dev" target="_blank" rel="noreferrer">
          <img src={viteLogo} className="logo" alt="Vite logo" />
        </a>
        <a href="https://react.dev" target="_blank" rel="noreferrer">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
        <a href="https://www.apollographql.com" target="_blank" rel="noreferrer">
          <img src={apolloLogo} className="logo apollo" alt="Apollo logo" />
        </a>
      </div>
      <h1>Vite + React + Apollo Client</h1>
      
      <div className="card">
        <button onClick={() => setCount((count) => count + 1)}>
          count is {count}
        </button>
        <p>
          Edit <code>src/App.tsx</code> and save to test HMR
        </p>
      </div>

      <div className="apollo-status">
        <h2>GraphQL Status</h2>
        {loading && <p>Loading...</p>}
        {error && (
          <div className="error">
            <p>Error connecting to GraphQL endpoint:</p>
            <code>{error.message}</code>
            <p>Make sure your GraphQL server is running at:</p>
            <code>{import.meta.env.VITE_APOLLO_ENDPOINT}</code>
          </div>
        )}
        {data && (
          <div className="success">
            <p>✅ Successfully connected to GraphQL!</p>
            <pre>{JSON.stringify(data, null, 2)}</pre>
          </div>
        )}
      </div>

      <p className="read-the-docs">
        Click on the Vite, React, and Apollo logos to learn more
      </p>
    </div>
  )
}

export default App
"#;
        fs::write(self.target_dir.join("src/App.tsx"), app_content)?;

        // Create CSS files
        self.create_css_files()?;

        // Create vite-env.d.ts
        let vite_env = r#"/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_APOLLO_ENDPOINT: string
  readonly VITE_APOLLO_GRAPH_REF: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
"#;
        fs::write(self.target_dir.join("src/vite-env.d.ts"), vite_env)?;

        Ok(())
    }

    fn create_css_files(&self) -> Result<()> {
        // App.css
        let app_css = r#"#root {
  max-width: 1280px;
  margin: 0 auto;
  padding: 2rem;
  text-align: center;
}

.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
  transition: filter 300ms;
}
.logo:hover {
  filter: drop-shadow(0 0 2em #646cffaa);
}
.logo.react:hover {
  filter: drop-shadow(0 0 2em #61dafbaa);
}
.logo.apollo:hover {
  filter: drop-shadow(0 0 2em #311c87aa);
}

@keyframes logo-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: no-preference) {
  a:nth-of-type(2) .logo {
    animation: logo-spin infinite 20s linear;
  }
}

.card {
  padding: 2em;
}

.read-the-docs {
  color: #888;
}

.apollo-status {
  margin-top: 2rem;
  padding: 1rem;
  border-radius: 8px;
  background: #f8f9fa;
}

.apollo-status h2 {
  margin-top: 0;
  color: #333;
}

.error {
  color: #d32f2f;
  background: #ffebee;
  padding: 1rem;
  border-radius: 4px;
  margin: 1rem 0;
}

.success {
  color: #2e7d32;
  background: #e8f5e8;
  padding: 1rem;
  border-radius: 4px;
  margin: 1rem 0;
}

.error code,
.success code {
  background: rgba(0, 0, 0, 0.1);
  padding: 0.2rem 0.4rem;
  border-radius: 3px;
  font-family: 'Courier New', monospace;
}

.success pre {
  background: rgba(0, 0, 0, 0.05);
  padding: 1rem;
  border-radius: 4px;
  overflow-x: auto;
  text-align: left;
  font-size: 0.9rem;
}
"#;
        fs::write(self.target_dir.join("src/App.css"), app_css)?;

        // index.css
        let index_css = r#":root {
  font-family: Inter, system-ui, Avenir, Helvetica, Arial, sans-serif;
  line-height: 1.5;
  font-weight: 400;

  color-scheme: light dark;
  color: rgba(255, 255, 255, 0.87);
  background-color: #242424;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

a {
  font-weight: 500;
  color: #646cff;
  text-decoration: inherit;
}
a:hover {
  color: #535bf2;
}

body {
  margin: 0;
  display: flex;
  place-items: center;
  min-width: 320px;
  min-height: 100vh;
}

h1 {
  font-size: 3.2em;
  line-height: 1.1;
}

button {
  border-radius: 8px;
  border: 1px solid transparent;
  padding: 0.6em 1.2em;
  font-size: 1em;
  font-weight: 500;
  font-family: inherit;
  background-color: #1a1a1a;
  color: inherit;
  cursor: pointer;
  transition: border-color 0.25s;
}
button:hover {
  border-color: #646cff;
}
button:focus,
button:focus-visible {
  outline: 4px auto -webkit-focus-ring-color;
}

@media (prefers-color-scheme: light) {
  :root {
    color: #213547;
    background-color: #ffffff;
  }
  a:hover {
    color: #747bff;
  }
  button {
    background-color: #f9f9f9;
  }
}
"#;
        fs::write(self.target_dir.join("src/index.css"), index_css)?;

        // Create simple placeholder SVGs
        let react_svg = "<svg width=\"32\" height=\"32\" viewBox=\"0 0 32 32\"><circle cx=\"16\" cy=\"16\" r=\"12\" fill=\"#61dafb\"/><text x=\"16\" y=\"20\" text-anchor=\"middle\" fill=\"white\" font-size=\"8\">React</text></svg>";
        fs::write(self.target_dir.join("src/assets/react.svg"), react_svg)?;

        let apollo_svg = "<svg width=\"32\" height=\"32\" viewBox=\"0 0 32 32\"><circle cx=\"16\" cy=\"16\" r=\"12\" fill=\"#311c87\"/><text x=\"16\" y=\"20\" text-anchor=\"middle\" fill=\"white\" font-size=\"7\">Apollo</text></svg>";
        fs::write(self.target_dir.join("src/assets/apollo.svg"), apollo_svg)?;

        let vite_svg = "<svg width=\"32\" height=\"32\" viewBox=\"0 0 32 32\"><circle cx=\"16\" cy=\"16\" r=\"12\" fill=\"#646cff\"/><text x=\"16\" y=\"20\" text-anchor=\"middle\" fill=\"white\" font-size=\"8\">Vite</text></svg>";
        fs::write(self.target_dir.join("public/vite.svg"), vite_svg)?;

        Ok(())
    }

    fn create_apollo_setup(&self) -> Result<()> {
        // apollo/client.ts
        let client_content = format!(
            r#"import {{ ApolloClient, InMemoryCache, createHttpLink, ApolloLink }} from '@apollo/client'
import {{ onError }} from '@apollo/client/link/error'

// Error handling
const errorLink = onError(({{ graphQLErrors, networkError }}) => {{
  if (graphQLErrors) {{
    graphQLErrors.forEach(({{ message, locations, path }}) =>
      console.error(
        `[GraphQL error]: Message: ${{message}}, Location: ${{locations}}, Path: ${{path}}`
      )
    )
  }}
  if (networkError) {{
    console.error(`[Network error]: ${{networkError}}`)
  }}
}})

// HTTP connection to the API
const httpLink = createHttpLink({{
  uri: import.meta.env.VITE_APOLLO_ENDPOINT || '{}',
}})

// Chain links
const link = ApolloLink.from([errorLink, httpLink])

// Apollo Client instance
export const client = new ApolloClient({{
  link,
  cache: new InMemoryCache({{
    typePolicies: {{
      Query: {{
        fields: {{
          // Add field policies here
        }},
      }},
    }},
  }}),
  defaultOptions: {{
    watchQuery: {{
      fetchPolicy: 'cache-and-network',
    }},
  }},
}})
"#,
            self.endpoint
        );

        fs::write(
            self.target_dir.join("src/apollo/client.ts"),
            client_content
        )?;

        // Example query
        let example_query = r#"import { gql } from '@apollo/client'

// This is an example query. Replace with your actual GraphQL query.
export const EXAMPLE_QUERY = gql`
  query ExampleQuery {
    __typename
  }
`

// Example of a typed query (when you have GraphQL code generation set up)
/*
export const GET_USERS = gql`
  query GetUsers {
    users {
      id
      name
      email
    }
  }
`
*/
"#;

        fs::write(
            self.target_dir.join("src/apollo/queries/example.ts"),
            example_query
        )?;

        // Example mutation
        let example_mutation = r#"import { gql } from '@apollo/client'

// Example mutation - replace with your actual mutations
export const EXAMPLE_MUTATION = gql`
  mutation ExampleMutation($input: String!) {
    exampleMutation(input: $input) {
      id
      success
    }
  }
`
"#;

        fs::write(
            self.target_dir.join("src/apollo/mutations/example.ts"),
            example_mutation
        )?;

        // Example fragment
        let example_fragment = r#"import { gql } from '@apollo/client'

// Example fragment - replace with your actual fragments
export const EXAMPLE_FRAGMENT = gql`
  fragment ExampleFragment on ExampleType {
    id
    name
    createdAt
  }
`
"#;

        fs::write(
            self.target_dir.join("src/apollo/fragments/example.ts"),
            example_fragment
        )?;

        Ok(())
    }

    fn create_env_files(&self) -> Result<()> {
        let env_content = format!(
            r#"# Apollo GraphQL Configuration
VITE_APOLLO_ENDPOINT={}
VITE_APOLLO_GRAPH_REF={}

# Add other environment variables here
"#,
            self.endpoint, self.graph_ref
        );

        fs::write(self.target_dir.join(".env"), env_content)?;

        let env_example = r#"# Apollo GraphQL Configuration
VITE_APOLLO_ENDPOINT=http://localhost:4000/graphql
VITE_APOLLO_GRAPH_REF=my-graph@current

# Add other environment variables here
"#;

        fs::write(self.target_dir.join(".env.example"), env_example)?;

        Ok(())
    }

    fn create_readme(&self) -> Result<()> {
        let readme = format!(
            r#"# {}

This project was bootstrapped with [Rover](https://www.apollographql.com/docs/rover/).

## Getting Started

### Prerequisites

- Node.js 16+ and npm installed on your machine
- Your GraphQL endpoint running at `{}`

### Installation

```bash
npm install
```

### Development

```bash
npm run dev
```

This will start the development server at [http://localhost:5173](http://localhost:5173).

### Building for Production

```bash
npm run build
```

### GraphQL Schema

To update your local GraphQL schema:

```bash
rover graph introspect {} --output schema.graphql
```

## Project Structure

```
src/
├── apollo/          # Apollo Client configuration and queries
│   ├── client.ts    # Apollo Client instance
│   ├── queries/     # GraphQL queries
│   ├── mutations/   # GraphQL mutations
│   └── fragments/   # GraphQL fragments
├── components/      # React components
├── hooks/          # Custom React hooks
├── pages/          # Page components
├── types/          # TypeScript type definitions
└── assets/         # Static assets
```

## Available Scripts

- `npm run dev` - Start development server
- `npm run build` - Build for production
- `npm run preview` - Preview production build
- `npm run lint` - Run ESLint
- `npm run type-check` - Run TypeScript compiler check
- `npm run codegen` - Introspect GraphQL schema

## Environment Variables

See `.env.example` for required environment variables.

## Learn More

- [Apollo Client Documentation](https://www.apollographql.com/docs/react/)
- [React Documentation](https://react.dev/)
- [Vite Documentation](https://vitejs.dev/)
- [Rover Documentation](https://www.apollographql.com/docs/rover/)
"#,
            self.project_name, self.endpoint, self.graph_ref
        );

        fs::write(self.target_dir.join("README.md"), readme)?;

        Ok(())
    }

    fn create_gitignore(&self) -> Result<()> {
        let gitignore = r#"# Logs
logs
*.log
npm-debug.log*
yarn-debug.log*
yarn-error.log*
pnpm-debug.log*
lerna-debug.log*

node_modules
dist
dist-ssr
*.local

# Editor directories and files
.vscode/*
!.vscode/extensions.json
.idea
.DS_Store
*.suo
*.ntvs*
*.njsproj
*.sln
*.sw?

# Environment variables
.env
.env.local
.env.*.local

# GraphQL
schema.graphql
__generated__
"#;

        fs::write(self.target_dir.join(".gitignore"), gitignore)?;

        Ok(())
    }

    fn create_eslint_config(&self) -> Result<()> {
        let eslint_config = serde_json::json!({
            "root": true,
            "env": { "browser": true, "es2020": true },
            "extends": [
                "eslint:recommended",
                "@typescript-eslint/recommended",
                "plugin:react-hooks/recommended",
            ],
            "ignorePatterns": ["dist", ".eslintrc.cjs"],
            "parser": "@typescript-eslint/parser",
            "plugins": ["react-refresh"],
            "rules": {
                "react-refresh/only-export-components": [
                    "warn",
                    { "allowConstantExport": true },
                ],
            },
        });

        fs::write(
            self.target_dir.join(".eslintrc.json"),
            serde_json::to_string_pretty(&eslint_config)?
        )?;

        Ok(())
    }
}