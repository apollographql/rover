use std::time::Duration;

use anyhow::anyhow;
use apollo_federation_types::config::{SchemaSource, SupergraphConfig};
use reqwest::Url;
use rover_client::blocking::StudioClient;

use crate::options::ProfileOpt;
use crate::{
    command::dev::{watcher::SubgraphSchemaWatcher, SupergraphOpts},
    utils::client::StudioClientConfig,
    RoverError, RoverResult,
};

impl SupergraphOpts {
    pub async fn get_subgraph_watchers(
        &self,
        client_config: &StudioClientConfig,
        supergraph_config: Option<SupergraphConfig>,
        polling_interval: u64,
        profile_opt: &ProfileOpt,
        subgraph_retries: u64,
    ) -> RoverResult<Option<Vec<SubgraphSchemaWatcher>>> {
        if supergraph_config.is_none() {
            return Ok(None);
        }

        let client = client_config
            .get_builder()
            .with_timeout(Duration::from_secs(5))
            .build()?;
        let mut studio_client: Option<StudioClient> = None;

        // WARNING: from here on I took the asynch branch's code; should be validated against main
        let mut res = Vec::new();
        for (yaml_subgraph_name, subgraph_config) in supergraph_config.unwrap().into_iter() {
            let routing_url = subgraph_config
                .routing_url
                .map(|url_str| Url::parse(&url_str).map_err(RoverError::from))
                .transpose()?;
            let elem = match subgraph_config.schema {
                SchemaSource::File { file } => {
                    let routing_url = routing_url.ok_or_else(|| {
                        anyhow!("`routing_url` must be set when using a local schema file")
                    })?;

                    SubgraphSchemaWatcher::new_from_file_path(
                        (yaml_subgraph_name, routing_url),
                        file,
                        subgraph_retries,
                    )
                }
                SchemaSource::SubgraphIntrospection {
                    subgraph_url,
                    introspection_headers,
                } => SubgraphSchemaWatcher::new_from_url(
                    (yaml_subgraph_name, subgraph_url.clone()),
                    client.clone(),
                    polling_interval,
                    introspection_headers,
                    subgraph_retries,
                    subgraph_url,
                ),
                SchemaSource::Sdl { sdl } => {
                    let routing_url = routing_url.ok_or_else(|| {
                        anyhow!("`routing_url` must be set when providing SDL directly")
                    })?;
                    SubgraphSchemaWatcher::new_from_sdl(
                        (yaml_subgraph_name, routing_url),
                        sdl,
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

                    SubgraphSchemaWatcher::new_from_graph_ref(
                        &graphref,
                        graphos_subgraph_name,
                        routing_url,
                        yaml_subgraph_name,
                        studio_client,
                        subgraph_retries,
                    )
                    .await
                }
            };
            res.push(elem?);
        }
        Ok(Some(res))
    }
}
