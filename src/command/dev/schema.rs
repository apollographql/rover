use std::net::SocketAddr;

use crate::{
    command::dev::{
        netstat::normalize_loopback_urls, protocol::SubgraphKeys, watcher::SubgraphSchemaWatcher,
    },
    error::RoverError,
    options::OptionalSubgraphOpts,
    Result, Suggestion,
};
use reqwest::{blocking::Client, Url};
use saucer::anyhow;

impl OptionalSubgraphOpts {
    pub fn get_subgraph_watcher(
        &self,
        socket_addr: &str,
        client: Client,
        session_subgraphs: Option<SubgraphKeys>,
        supergraph_socket_addr: SocketAddr,
    ) -> Result<SubgraphSchemaWatcher> {
        let url = self.prompt_for_url()?;
        let normalized_user_urls = normalize_loopback_urls(&url);
        let normalized_supergraph_urls = normalize_loopback_urls(
            &Url::parse(&format!("http://{}", supergraph_socket_addr)).unwrap(),
        );

        for normalized_user_url in &normalized_user_urls {
            for normalized_supergraph_url in &normalized_supergraph_urls {
                if normalized_supergraph_url == normalized_user_url {
                    let mut err = RoverError::new(anyhow!("The subgraph argument `--url {}` conflicts with the supergraph argument `--port {}`", &url, normalized_supergraph_url.port().unwrap()));
                    if session_subgraphs.is_none() {
                        err.set_suggestion(Suggestion::Adhoc("Set the `--port` flag to a different port to start the local supergraph.".to_string()))
                    } else {
                        err.set_suggestion(Suggestion::Adhoc("Start your subgraph on a different port and re-run this command with the new `--url`.".to_string()))
                    }
                    return Err(err);
                }
            }
        }

        let name = self.prompt_for_name()?;
        let schema = self.prompt_for_schema()?;

        let mut is_main_session = true;

        if let Some(session_subgraphs) = session_subgraphs {
            is_main_session = false;
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
                socket_addr,
                (name, url),
                schema,
                is_main_session,
            )
        } else {
            SubgraphSchemaWatcher::new_from_url(socket_addr, (name, url), client, is_main_session)
        }
    }
}
