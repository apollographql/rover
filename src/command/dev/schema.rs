use std::{net::SocketAddr, time::Duration};

use anyhow::anyhow;
use apollo_federation_types::config::{SchemaSource, SubgraphConfig, SupergraphConfig};
use reqwest::Url;

use rover_client::blocking::StudioClient;

use crate::options::ProfileOpt;
use crate::{
    command::dev::{
        netstat::normalize_loopback_urls, protocol::SubgraphWatcherMessenger, subgraph::Watcher,
        SupergraphOpts,
    },
    options::OptionalSubgraphOpts,
    utils::client::StudioClientConfig,
    RoverError, RoverErrorSuggestion, RoverResult,
};

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
        messenger: SubgraphWatcherMessenger,
        polling_interval: u64,
        profile_opt: &ProfileOpt,
        subgraph_retries: u64,
    ) -> RoverResult<Vec<Watcher>> {
        let client = client_config
            .get_builder()
            .with_timeout(Duration::from_secs(5))
            .build()?;
        let mut studio_client: Option<StudioClient> = None;

        let mut res = Vec::new();
        for (yaml_subgraph_name, subgraph_config) in supergraph_config.into_iter() {
            let routing_url = subgraph_config
                .routing_url
                .map(|url_str| Url::parse(&url_str).map_err(RoverError::from))
                .transpose()?;
            let elem = match subgraph_config.schema {
                SchemaSource::File { file } => {
                    let routing_url = routing_url.ok_or_else(|| {
                        anyhow!("`routing_url` must be set when using a local schema file")
                    })?;

                    Watcher::new_from_file_path(
                        yaml_subgraph_name,
                        routing_url,
                        file,
                        messenger.clone(),
                        subgraph_retries,
                    )
                }
                SchemaSource::SubgraphIntrospection {
                    subgraph_url,
                    introspection_headers,
                } => Watcher::new_from_url(
                    yaml_subgraph_name,
                    subgraph_url.clone(),
                    client.clone(),
                    messenger.clone(),
                    polling_interval,
                    introspection_headers,
                    subgraph_retries,
                    subgraph_url,
                ),
                SchemaSource::Sdl { sdl } => {
                    let routing_url = routing_url.ok_or_else(|| {
                        anyhow!("`routing_url` must be set when providing SDL directly")
                    })?;
                    Watcher::new_from_sdl(
                        yaml_subgraph_name,
                        routing_url,
                        sdl,
                        messenger.clone(),
                        subgraph_retries,
                    )
                }
                SchemaSource::Subgraph {
                    graphref,
                    subgraph: graphos_subgraph_name,
                } => {
                    let studio_client = if let Some(studio_client) = studio_client.as_ref() {
                        studio_client
                    } else {
                        let client = client_config.get_authenticated_client(profile_opt)?;
                        studio_client = Some(client);
                        studio_client.as_ref().unwrap()
                    };

                    Watcher::new_from_graph_ref(
                        &graphref,
                        graphos_subgraph_name,
                        routing_url,
                        yaml_subgraph_name,
                        messenger.clone(),
                        studio_client,
                        subgraph_retries,
                    )
                    .await
                }
            };
            res.push(elem?);
        }
        Ok(res)
    }
}
