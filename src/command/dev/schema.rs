use std::{net::SocketAddr, time::Duration};

use crate::{
    command::dev::{
        netstat::normalize_loopback_urls, protocol::FollowerMessenger,
        watcher::SubgraphSchemaWatcher,
    },
    options::OptionalSubgraphOpts,
    utils::client::StudioClientConfig,
    RoverError, RoverErrorSuggestion, RoverResult,
};
use anyhow::anyhow;
use reqwest::Url;

impl OptionalSubgraphOpts {
    pub fn get_subgraph_watcher(
        &self,
        router_socket_addr: SocketAddr,
        client_config: &StudioClientConfig,
        follower_messenger: FollowerMessenger,
    ) -> RoverResult<SubgraphSchemaWatcher> {
        let client = client_config
            .get_builder()
            .with_timeout(Duration::from_secs(5))
            .build()?;
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
            SubgraphSchemaWatcher::new_from_url(
                (name, url),
                client,
                follower_messenger,
                self.subgraph_polling_interval,
            )
        }
    }
}
