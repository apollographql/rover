use reqwest;
use serde::Deserialize;
use anyhow::Result;
use std::collections::HashMap;
use futures::future::join_all;

#[derive(Deserialize)]
struct NpmPackageInfo {
    #[serde(rename = "dist-tags")]
    dist_tags: DistTags,
    #[allow(dead_code)]
    versions: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct DistTags {
    latest: String,
}

#[derive(Default)]
pub struct DependencyVersions {
    pub react: String,
    pub react_dom: String,
    pub apollo_client: String,
    pub graphql: String,
    pub vite: String,
    pub vite_plugin_react: String,
    pub typescript: String,
    pub types_react: String,
    pub types_react_dom: String,
    pub typescript_eslint_plugin: String,
    pub typescript_eslint_parser: String,
    pub eslint: String,
    pub eslint_plugin_react_hooks: String,
    pub eslint_plugin_react_refresh: String,
}

impl DependencyVersions {
    pub fn with_defaults() -> Self {
        Self {
            react: "^18.2.0".to_string(),
            react_dom: "^18.2.0".to_string(),
            apollo_client: "^3.8.0".to_string(),
            graphql: "^16.8.0".to_string(),
            vite: "^5.0.0".to_string(),
            vite_plugin_react: "^4.2.0".to_string(),
            typescript: "^5.2.0".to_string(),
            types_react: "^18.2.56".to_string(),
            types_react_dom: "^18.2.19".to_string(),
            typescript_eslint_plugin: "^7.0.2".to_string(),
            typescript_eslint_parser: "^7.0.2".to_string(),
            eslint: "^8.56.0".to_string(),
            eslint_plugin_react_hooks: "^4.6.0".to_string(),
            eslint_plugin_react_refresh: "^0.4.5".to_string(),
        }
    }
}

pub struct SafeNpmClient {
    client: reqwest::Client,
}

impl SafeNpmClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent("rover-cli")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        
        Self { client }
    }

    /// Get latest version of a package without spawning npm
    pub async fn get_latest_version(&self, package: &str) -> Result<String> {
        let url = format!("https://registry.npmjs.org/{}", package);
        let resp = self.client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;
        
        let info: NpmPackageInfo = resp.json().await?;
        Ok(info.dist_tags.latest)
    }

    /// Get package.json dependencies for latest versions
    pub async fn get_latest_deps(&self) -> Result<DependencyVersions> {
        let mut deps = DependencyVersions::with_defaults();
        
        // List of packages to fetch latest versions for
        let packages = vec![
            "react",
            "react-dom",
            "@apollo/client",
            "graphql",
            "vite",
            "@vitejs/plugin-react",
            "typescript",
            "@types/react",
            "@types/react-dom",
            "@typescript-eslint/eslint-plugin",
            "@typescript-eslint/parser",
            "eslint",
            "eslint-plugin-react-hooks",
            "eslint-plugin-react-refresh",
        ];

        // Fetch versions in parallel
        let futures: Vec<_> = packages.iter()
            .map(|pkg| self.get_latest_version(pkg))
            .collect();

        let results = join_all(futures).await;

        // Update versions based on results
        for (i, result) in results.into_iter().enumerate() {
            if let Ok(version) = result {
                let version_str = format!("^{}", version);
                match packages[i] {
                    "react" => deps.react = version_str,
                    "react-dom" => deps.react_dom = version_str,
                    "@apollo/client" => deps.apollo_client = version_str,
                    "graphql" => deps.graphql = version_str,
                    "vite" => deps.vite = version_str,
                    "@vitejs/plugin-react" => deps.vite_plugin_react = version_str,
                    "typescript" => deps.typescript = version_str,
                    "@types/react" => deps.types_react = version_str,
                    "@types/react-dom" => deps.types_react_dom = version_str,
                    "@typescript-eslint/eslint-plugin" => deps.typescript_eslint_plugin = version_str,
                    "@typescript-eslint/parser" => deps.typescript_eslint_parser = version_str,
                    "eslint" => deps.eslint = version_str,
                    "eslint-plugin-react-hooks" => deps.eslint_plugin_react_hooks = version_str,
                    "eslint-plugin-react-refresh" => deps.eslint_plugin_react_refresh = version_str,
                    _ => {}
                }
            }
        }

        Ok(deps)
    }

    /// Get dependency versions with fallback to defaults
    pub async fn get_deps_with_fallback(&self) -> DependencyVersions {
        match self.get_latest_deps().await {
            Ok(deps) => deps,
            Err(e) => {
                eprintln!("Warning: Could not fetch latest versions: {}. Using defaults.", e);
                DependencyVersions::with_defaults()
            }
        }
    }
}

impl Default for SafeNpmClient {
    fn default() -> Self {
        Self::new()
    }
}