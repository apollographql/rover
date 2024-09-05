use std::{net::SocketAddr, time::Duration};

use anyhow::anyhow;
use apollo_federation_types::config::{SchemaSource, SupergraphConfig};
use reqwest::Url;

use rover_client::blocking::StudioClient;

use crate::options::ProfileOpt;
use crate::{
    command::dev::{
        netstat::normalize_loopback_urls, protocol::FollowerMessenger,
        watcher::SubgraphSchemaWatcher, SupergraphOpts,
    },
    options::OptionalSubgraphOpts,
    utils::client::StudioClientConfig,
    RoverError, RoverErrorSuggestion, RoverResult,
};

impl OptionalSubgraphOpts {
    pub fn get_subgraph_watcher(
        &self,
        router_socket_addr: SocketAddr,
        client_config: &StudioClientConfig,
        follower_messenger: FollowerMessenger,
    ) -> RoverResult<SubgraphSchemaWatcher> {
        tracing::info!("checking for existing subgraphs");
        let session_subgraphs = follower_messenger.session_subgraphs()?;
        let url = self.prompt_for_url()?;
        let normalized_user_urls = normalize_loopback_urls(&url);
        let normalized_supergraph_urls = normalize_loopback_urls(
            &Url::parse(&format!("http://{}", router_socket_addr)).unwrap(),
        );

        for normalized_user_url in &normalized_user_urls {
            for normalized_supergraph_url in &normalized_supergraph_urls {
                if normalized_supergraph_url == normalized_user_url {
                    let mut err = RoverError::new(anyhow!("The subgraph argument `--url {}` conflicts with the supergraph argument `--supergraph-port {}`", &url, normalized_supergraph_url.port().unwrap()));
                    if session_subgraphs.is_none() {
                        err.set_suggestion(RoverErrorSuggestion::Adhoc("Set the `--supergraph-port` flag to a different port to start the local supergraph.".to_string()))
                    } else {
                        err.set_suggestion(RoverErrorSuggestion::Adhoc("Start your subgraph on a different port and re-run this command with the new `--url`.".to_string()))
                    }
                    return Err(err);
                }
            }
        }

        let name = self.prompt_for_name()?;
        let schema = self.prompt_for_schema()?;

        if let Some(session_subgraphs) = session_subgraphs {
            for (session_subgraph_name, session_subgraph_url) in session_subgraphs {
                if session_subgraph_name == name {
                    return Err(RoverError::new(anyhow!(
                        "subgraph with name '{}' is already running in this `rover dev` session",
                        &name
                    )));
                }
                let normalized_session_urls = normalize_loopback_urls(&session_subgraph_url);
                for normalized_user_url in &normalized_user_urls {
                    for normalized_session_url in &normalized_session_urls {
                        if normalized_session_url == normalized_user_url {
                            return Err(RoverError::new(anyhow!(
                                "subgraph with url '{}' is already running in this `rover dev` session",
                                &url
                            )));
                        }
                    }
                }
            }
        }

        if let Some(schema) = schema {
            SubgraphSchemaWatcher::new_from_file_path(
                (name, url),
                schema,
                follower_messenger,
                self.subgraph_retries,
            )
        } else {
            let client = client_config
                .get_builder()
                .with_timeout(Duration::from_secs(5))
                .build()?;
            SubgraphSchemaWatcher::new_from_url(
                (name, url.clone()),
                client,
                follower_messenger,
                self.subgraph_polling_interval,
                None,
                self.subgraph_retries,
                url,
            )
        }
    }
}

impl SupergraphOpts {
    pub async fn get_subgraph_watchers(
        &self,
        client_config: &StudioClientConfig,
        supergraph_config: Option<SupergraphConfig>,
        follower_messenger: FollowerMessenger,
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
                        follower_messenger.clone(),
                        subgraph_retries,
                    )
                }
                SchemaSource::SubgraphIntrospection {
                    subgraph_url,
                    introspection_headers,
                } => SubgraphSchemaWatcher::new_from_url(
                    (yaml_subgraph_name, subgraph_url.clone()),
                    client.clone(),
                    follower_messenger.clone(),
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
                        follower_messenger.clone(),
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
                        follower_messenger.clone(),
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
