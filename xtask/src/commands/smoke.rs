use std::collections::{HashMap, HashSet};
use std::process::{Child, Command};
use std::time::Duration;

use anyhow::{anyhow, Error};
use camino::Utf8PathBuf;
use clap::Parser;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::time::Instant;

use crate::tools::NpmRunner;
use crate::utils::PKG_PROJECT_ROOT;

#[derive(Debug, Parser)]
pub struct Smoke {
    #[arg(long = "binary-path")]
    pub(crate) binary_path: Utf8PathBuf,
    #[arg(long = "federation-version")]
    pub(crate) federation_version: Option<String>,
    #[arg(long = "router-version")]
    pub(crate) router_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReducedSuperGraphConfig {
    subgraphs: HashMap<String, ReducedSubgraphConfig>,
}
#[derive(Debug, Deserialize)]
struct ReducedSubgraphConfig {
    routing_url: String,
}

impl ReducedSuperGraphConfig {
    pub fn get_subgraph_urls(self) -> Vec<String> {
        self.subgraphs
            .values()
            .into_iter()
            .map(|x| x.routing_url.clone())
            .collect()
    }
}

const SUBGRAPH_TIMEOUT_DURATION: Duration = Duration::from_secs(10);
const ROUTER_TIMEOUT_DURATION: Duration = Duration::from_secs(10);

impl Smoke {
    pub async fn run(&self) -> anyhow::Result<()> {
        // Spin up the subgraphs first
        crate::info!("Start subgraphs running...");
        let npm_runner = NpmRunner::new()?;
        let mut subgraph_handle = npm_runner.run_subgraphs()?;
        // Wait for the subgraphs to respond correctly to an introspection request
        crate::info!("Wait for subgraphs to become available...");
        let client = Client::new();
        Self::wait_for_subgraphs(&client).await?;
        // Invoke Rover Dev
        let port = 4000;
        crate::info!("Run rover dev on port {}...", port);
        let mut rover_dev_handle = self.run_rover_dev(
            port,
            self.federation_version.clone(),
            self.router_version.clone(),
        )?;
        // Wait polling the router endpoint until that returns something sensible or times out
        crate::info!("Wait for router to return a response...");
        Self::wait_for_router(&client, port).await?;
        // Close Up The Resources
        crate::info!("Clean up resources...");
        rover_dev_handle.kill()?;
        subgraph_handle.kill()?;
        Ok(())
    }

    async fn wait_for_router(client: &Client, port: u64) -> Result<(), Error> {
        let federated_query = json!({"query": "{allPandas{    name    favoriteFood  }  allProducts {    createdBy {      email      name    }    package    sku  }}"});
        let federated_response = json!({"data":{"allPandas":[{"name":"Basi","favoriteFood":"bamboo leaves"},{"name":"Yun","favoriteFood":"apple"}],"allProducts":[{"createdBy":{"email":"mara@acmecorp.com","name":"Mara"},"package":"@apollo/federation","sku":"federation"},{"createdBy":{"email":"mara@acmecorp.com","name":"Mara"},"package":"","sku":"studio"}]}});

        let start = Instant::now();
        loop {
            let res = client
                .post(format!("http://localhost:{}", port))
                .json(&federated_query)
                .send()
                .await;
            match res {
                Ok(res) => {
                    if res.status().is_success() {
                        assert_eq!(res.json::<Value>().await?, federated_response);
                        return Ok(());
                    }
                }
                Err(_) => {
                    if start.elapsed() >= ROUTER_TIMEOUT_DURATION {
                        crate::info!(
                            "Could not connect to supergraph on port {} - Exiting...",
                            port
                        );
                        return Err(anyhow!("Failed to connect to supergraph.").into());
                    }
                    crate::info!(
                        "Could not connect to supergraph on port {} - Will retry",
                        port
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            }
        }
    }

    fn run_rover_dev(
        &self,
        port: u64,
        federation_version: Option<String>,
        router_version: Option<String>,
    ) -> Result<Child, Error> {
        let project_root = PKG_PROJECT_ROOT.clone();
        let supergraph_demo_directory = project_root.join("examples").join("supergraph-demo");
        let mut cmd = Command::new(&self.binary_path.canonicalize_utf8().unwrap());
        cmd.args([
            "dev",
            "--supergraph-config",
            "supergraph.yaml",
            "--router-config",
            "router.yaml",
            "--supergraph-port",
            &format!("{}", port),
            "--elv2-license",
            "accept",
        ])
        .current_dir(supergraph_demo_directory);
        if let Some(version) = federation_version {
            cmd.env("APOLLO_ROVER_DEV_COMPOSITION_VERSION", version);
        };
        if let Some(version) = router_version {
            cmd.env("APOLLO_ROVER_DEV_ROUTER_VERSION", version);
        };
        let rover_dev_handle = cmd.spawn()?;
        Ok(rover_dev_handle)
    }

    async fn wait_for_subgraphs(client: &Client) -> Result<(), Error> {
        let introspection_query = json!({"query": "{__schema{types{name}}}"});
        let mut finished = HashSet::new();

        // Read in the supergraph YAML file, so we can extract the routing URLs, this way we
        // don't need to hardcode any of the port values etc.
        let project_root = PKG_PROJECT_ROOT.clone();
        let supergraph_yaml_path = project_root
            .join("examples")
            .join("supergraph-demo/supergraph.yaml");
        let urls = Self::get_subgraph_urls(supergraph_yaml_path);

        // Loop over the URLs
        let start = Instant::now();
        loop {
            for url in urls.iter() {
                let res = client.post(url).json(&introspection_query).send().await;
                match res {
                    Ok(res) => {
                        if res.status().is_success() {
                            finished.insert(url);
                        }
                    }
                    Err(e) => {
                        crate::info!(
                            "Could not connect to subgraph on {}: {:} - Will retry",
                            url,
                            e
                        );
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }

            if finished.len() == urls.len() {
                return Ok(());
            }
            if start.elapsed() >= SUBGRAPH_TIMEOUT_DURATION {
                crate::info!("Could not connect to all subgraphs. Exiting...");
                return Err(anyhow!("Could not connect to all subgraphs"));
            }
        }
    }

    fn get_subgraph_urls(supergraph_yaml_path: Utf8PathBuf) -> Vec<String> {
        let content = std::fs::read_to_string(supergraph_yaml_path).unwrap();
        let sc_config: ReducedSuperGraphConfig = serde_yaml::from_str(&content).unwrap();
        sc_config.get_subgraph_urls()
    }
}
