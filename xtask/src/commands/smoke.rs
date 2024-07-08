use std::collections::HashSet;
use std::process::Command;
use std::time::Duration;

use camino::Utf8PathBuf;
use clap::Parser;
use serde_json::{json, Value};

use crate::tools::NpmRunner;
use crate::utils::PKG_PROJECT_ROOT;

#[derive(Debug, Parser)]
pub struct Smoke {
    #[arg(long = "binary_path")]
    pub(crate) binary_path: Utf8PathBuf,
}

impl Smoke {
    pub async fn run(&self) -> anyhow::Result<()> {
        // Spin up the subgraphs first
        let npm_runner = NpmRunner::new()?;
        let mut subgraph_handle = npm_runner.run_subgraphs()?;
        // Wait polling the endpoints of the three services until they return something sensible
        let client = reqwest::Client::new();
        let introspection_query = json!({"query": "{__schema{types{name}}}"});
        let mut finished = HashSet::new();
        let ports = vec!["4001", "4002", "4003"];
        loop {
            for port in ports.iter() {
                let res = client
                    .post(format!("http://localhost:{}", port))
                    .json(&introspection_query)
                    .send()
                    .await;
                match res {
                    Ok(res) => {
                        if res.status().is_success() {
                            finished.insert(port);
                        }
                    }
                    Err(e) => {
                        crate::info!(
                            "Could not connect to subgraph on port {}: {:} - Will retry",
                            port,
                            e
                        );
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
            if finished.len() == ports.len() {
                break;
            }
        }

        // Invoke Rover Dev
        let project_root = PKG_PROJECT_ROOT.clone();
        let supergraph_demo_directory = project_root.join("examples").join("supergraph-demo");
        let mut cmd = Command::new(&self.binary_path);
        cmd.args([
            "dev",
            "--supergraph-config",
            "supergraph.yaml",
            "--router-config",
            "router.yaml",
        ])
        .current_dir(supergraph_demo_directory);
        let mut rover_dev_handle = cmd.spawn()?;

        // Wait polling the router endpoint until that returns something sensible or times out
        let federated_query = json!({"query": "{allPandas{    name    favoriteFood  }  allProducts {    createdBy {      email      name    }    package    sku  }}"});
        let federated_response = json!({"data":{"allPandas":[{"name":"Basi","favoriteFood":"bamboo leaves"},{"name":"Yun","favoriteFood":"apple"}],"allProducts":[{"createdBy":{"email":"mara@acmecorp.com","name":"Mara"},"package":"@apollo/federation","sku":"federation"},{"createdBy":{"email":"mara@acmecorp.com","name":"Mara"},"package":"","sku":"studio"}]}});
        loop {
            let res = client
                .post("http://localhost:4000")
                .json(&federated_query)
                .send()
                .await;
            match res {
                Ok(res) => {
                    if res.status().is_success() {
                        assert_eq!(res.json::<Value>().await?, federated_response);
                        break;
                    }
                }
                Err(e) => {
                    crate::info!(
                        "Could not connect to supergraph on port 4000: {:} - Will retry",
                        e
                    );
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            }
        }

        // Close Up The Resources
        rover_dev_handle.kill()?;
        subgraph_handle.kill()?;
        Ok(())
    }
}
