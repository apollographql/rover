use std::{net::SocketAddr, time::Duration};

use anyhow::anyhow;
use apollo_federation_types::config::SchemaSource;
use reqwest::Url;
use rover_std::Fs;

use crate::command::supergraph::expand_supergraph_yaml;
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
        tracing::info!("checking version");
        follower_messenger.version_check()?;
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
            SubgraphSchemaWatcher::new_from_file_path((name, url), schema, follower_messenger)
        } else {
            let client = client_config
                .get_builder()
                .with_timeout(Duration::from_secs(5))
                .build()?;
            SubgraphSchemaWatcher::new_from_url(
                (name, url),
                client,
                follower_messenger,
                self.subgraph_polling_interval,
                None,
            )
        }
    }
}

impl SupergraphOpts {
    pub fn get_subgraph_watchers(
        &self,
        client_config: &StudioClientConfig,
        follower_messenger: FollowerMessenger,
        polling_interval: u64,
    ) -> RoverResult<Option<Vec<SubgraphSchemaWatcher>>> {
        let config_path = if let Some(path) = &self.supergraph_config_path {
            path
        } else {
            return Ok(None);
        };

        tracing::info!("checking version");
        follower_messenger.version_check()?;

        let config_content = Fs::read_file(config_path)?;
        let supergraph_config = expand_supergraph_yaml(&config_content)?;

        let client = client_config
            .get_builder()
            .with_timeout(Duration::from_secs(5))
            .build()?;
        supergraph_config
            .into_iter()
            .map(|(name, subgraph_config)| {
                let routing_url = subgraph_config
                    .routing_url
                    .ok_or_else(|| {
                        RoverError::new(anyhow!("routing_url must be declared for every subgraph"))
                    })
                    .and_then(|url_str| Url::parse(&url_str).map_err(RoverError::from))?;
                match subgraph_config.schema {
                    SchemaSource::File { file } => SubgraphSchemaWatcher::new_from_file_path(
                        (name, routing_url),
                        file,
                        follower_messenger.clone(),
                    ),
                    SchemaSource::SubgraphIntrospection {
                        subgraph_url,
                        introspection_headers,
                    } => SubgraphSchemaWatcher::new_from_url(
                        (name, subgraph_url),
                        client.clone(),
                        follower_messenger.clone(),
                        polling_interval,
                        introspection_headers,
                    ),
                    SchemaSource::Sdl { .. } | SchemaSource::Subgraph { .. } => {
                        Err(RoverError::new(anyhow!(
                            "Detected an invalid `graphref` or `sdl` schema source in {file}. rover dev only supports sourcing schemas via introspection and schema files. see https://www.apollographql.com/docs/rover/commands/supergraphs/#yaml-configuration-file for more information."
                        )))
                    }
                }
            })
            .collect::<RoverResult<Vec<_>>>()
            .map(Some)
    }
}
