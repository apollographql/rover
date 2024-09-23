use std::{net::SocketAddr, time::Duration};

use crate::command::dev::subgraph::SubgraphUpdated;
use crate::{
    command::dev::{netstat::normalize_loopback_urls, subgraph::Watcher, SupergraphOpts},
    options::OptionalSubgraphOpts,
    utils::client::StudioClientConfig,
    RoverError, RoverErrorSuggestion, RoverResult,
};
use anyhow::anyhow;
use apollo_federation_types::config::{SchemaSource, SubgraphConfig, SupergraphConfig};
use reqwest::Url;
use tokio::sync::mpsc::Sender;

impl OptionalSubgraphOpts {
    pub fn get_single_subgraph_from_opts(
        &self,
        router_socket_addr: SocketAddr,
    ) -> RoverResult<SupergraphConfig> {
        let url = self.prompt_for_url()?;
        let normalized_user_urls = normalize_loopback_urls(&url);
        let normalized_supergraph_urls =
            normalize_loopback_urls(&Url::parse(&format!("http://{}", router_socket_addr))?);

        for normalized_user_url in &normalized_user_urls {
            for normalized_supergraph_url in &normalized_supergraph_urls {
                if normalized_supergraph_url == normalized_user_url {
                    let mut err = RoverError::new(anyhow!("The subgraph argument `--url {}` conflicts with the supergraph argument `--supergraph-port {}`", &url, normalized_supergraph_url.port().unwrap()));
                    err.set_suggestion(RoverErrorSuggestion::Adhoc("Set the `--supergraph-port` flag to a different port to start the local supergraph.".to_string()));
                    return Err(err);
                }
            }
        }

        let name = self.prompt_for_name()?;
        let schema = self.prompt_for_schema()?;
        let routing_url = Some(url.to_string());

        let schema = if let Some(schema) = schema {
            SchemaSource::File { file: schema }
        } else {
            SchemaSource::SubgraphIntrospection {
                subgraph_url: url,
                introspection_headers: None,
            }
        };
        let subgraph_config = SubgraphConfig {
            routing_url,
            schema,
        };
        Ok(SupergraphConfig::new(
            [(name, subgraph_config)].into_iter().collect(),
            None,
        ))
    }
}

impl SupergraphOpts {
    pub async fn get_subgraph_watchers(
        &self,
        client_config: &StudioClientConfig,
        supergraph_config: SupergraphConfig,
        messenger: Sender<SubgraphUpdated>,
        polling_interval: u64,
        subgraph_retries: u64,
    ) -> RoverResult<Vec<Watcher>> {
        let client = client_config
            .get_builder()
            .with_timeout(Duration::from_secs(5))
            .build()?;

        let watchers = supergraph_config
            .into_iter()
            .filter_map(|(subgraph_name, subgraph_config)| {
                match subgraph_config.schema {
                    SchemaSource::File { file } => Some(Watcher::new_from_file_path(
                        subgraph_name,
                        file,
                        messenger.clone(),
                        subgraph_retries,
                    )),
                    SchemaSource::SubgraphIntrospection {
                        subgraph_url,
                        introspection_headers,
                    } => Some(Watcher::new_from_url(
                        subgraph_name,
                        client.clone(),
                        messenger.clone(),
                        polling_interval,
                        introspection_headers,
                        subgraph_retries,
                        subgraph_url,
                    )),
                    SchemaSource::Sdl { .. } | SchemaSource::Subgraph { .. } => {
                        // We don't watch these
                        None
                    }
                }
            })
            .collect();
        Ok(watchers)
    }
}
