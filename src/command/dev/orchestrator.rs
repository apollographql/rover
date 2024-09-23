use anyhow::anyhow;
use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use futures::TryFutureExt;
use std::str::FromStr;
use std::{fmt::Debug, net::TcpListener};
use tracing::{info, warn};

use super::protocol::{CompositionResult, SubgraphMessageChannel, SubgraphName, SubgraphUpdated};
use crate::federation::supergraph_config::{ResolvedSubgraphConfig, ResolvedSupergraphConfig};
use crate::{
    command::dev::{
        compose::ComposeRunner,
        do_dev::log_err_and_continue,
        router::{RouterConfigHandler, RouterRunner},
        OVERRIDE_DEV_COMPOSITION_VERSION,
    },
    options::PluginOpts,
    utils::client::StudioClientConfig,
    RoverError, RoverErrorSuggestion, RoverResult,
};

/// The top-level runner which handles router, recomposition of supergraphs, and wrangling the various `SubgraphWatcher`s
#[derive(Debug)]
pub(crate) struct Orchestrator {
    supergraph: ResolvedSupergraphConfig,
    compose_runner: ComposeRunner,
    router_runner: Option<RouterRunner>,
    subgraph_updates: SubgraphMessageChannel,
}

impl Orchestrator {
    /// Create a new [`LeaderSession`] that is responsible for running composition and the router
    /// It listens on a socket for incoming messages for subgraph changes, in addition to watching
    /// its own subgraph
    /// Returns:
    /// Ok(Some(Self)) when successfully initiated
    /// Ok(None) when a LeaderSession already exists for that address
    /// Err(RoverError) when something went wrong.
    pub async fn new(
        override_install_path: Option<Utf8PathBuf>,
        client_config: &StudioClientConfig,
        subgraph_updates: SubgraphMessageChannel,
        plugin_opts: PluginOpts,
        mut supergraph: ResolvedSupergraphConfig,
        router_config_handler: RouterConfigHandler,
        license: Option<Utf8PathBuf>,
    ) -> RoverResult<Self> {
        let raw_socket_name = router_config_handler.get_raw_socket_name();
        let router_socket_addr = router_config_handler.get_router_address();

        // if we can't connect to the socket, we should start it and listen for incoming
        // subgraph events
        //
        // remove the socket file before starting in case it was here from last time
        // if we can't connect to it, it's safe to remove
        let _ = std::fs::remove_file(&raw_socket_name);

        if TcpListener::bind(router_socket_addr).is_err() {
            let mut err =
                RoverError::new(anyhow!("You cannot bind the router to '{}' because that address is already in use by another process on this machine.", &router_socket_addr));
            err.set_suggestion(RoverErrorSuggestion::Adhoc(
                format!("Try setting a different port for the router to bind to with the `--supergraph-port` argument, or shut down the process bound to '{}'.", &router_socket_addr)
            ));
            return Err(err);
        }

        if let Some(version_from_env) = Self::get_federation_version_from_env() {
            supergraph.federation_version = version_from_env;
        };

        // create a [`ComposeRunner`] that will be in charge of composing our supergraph
        let compose_runner = ComposeRunner::new(
            plugin_opts.clone(),
            override_install_path.clone(),
            client_config.clone(),
            router_config_handler.get_supergraph_schema_path(),
        )
        .await?;

        // create a [`RouterRunner`] that we will use to spawn the router when we have a successful composition
        let mut router_runner = RouterRunner::new(
            router_config_handler.get_supergraph_schema_path(),
            router_config_handler.get_router_config_path(),
            plugin_opts.clone(),
            router_socket_addr,
            router_config_handler.get_router_listen_path(),
            override_install_path,
            client_config.clone(),
            license,
        );

        // install plugins before proceeding
        router_runner.maybe_install_router().await?;
        router_config_handler.start()?;

        let mut orchestrator = Self {
            supergraph,
            compose_runner,
            router_runner: Some(router_runner),
            subgraph_updates,
        };
        orchestrator
            .compose()
            .await
            .map_err(|err| anyhow!("Failed to compose supergraph: {}", err))?;
        Ok(orchestrator)
    }

    /// If the user sets the federation version as an environment variable, use that instead of
    /// the version in supergraph config
    fn get_federation_version_from_env() -> Option<FederationVersion> {
        OVERRIDE_DEV_COMPOSITION_VERSION
            .as_ref()
            .and_then(|version_str| {
                match FederationVersion::from_str(&format!("={}", version_str)) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        warn!("could not parse version from environment variable '{:}'", e);
                        info!("will check supergraph schema next...");
                        None
                    }
                }
            })
    }

    /// Listen for incoming subgraph updates and re-compose the supergraph
    pub(crate) async fn receive_all_subgraph_updates(
        &mut self,
        mut ready_sender: futures::channel::mpsc::Sender<()>,
    ) -> ! {
        ready_sender.try_send(()).unwrap();
        loop {
            tracing::trace!("main session waiting for follower message");
            let message = self.subgraph_updates.receiver.recv().unwrap();
            self.handle_subgraph_message(message).await;
        }
    }

    // TODO: handle this in composer once we watch supergraph
    // /// Adds a subgraph to the internal supergraph representation.
    // async fn add_subgraph(&mut self, subgraph_entry: &SubgraphEntry) {
    //     let is_first_subgraph = self.supergraph.subgraphs.is_empty();
    //     let ((name, url), sdl) = subgraph_entry;
    //
    //     if let Vacant(e) = self
    //         .supergraph
    //         .subgraphs
    //         .entry((name.to_string(), url.clone()))
    //     {
    //         e.insert(sdl.to_string());
    //
    //         // Followers add subgraphs, but sometimes those subgraphs depend on each other
    //         // (e.g., through extending a type in another subgraph). When that happens,
    //         // composition fails until _all_ subgraphs are loaded in. This acknowledges the
    //         // follower's message when we haven't loaded in all the subgraphs, deferring
    //         // composition until we have at least the number of subgraphs represented in the
    //         // supergraph.yaml file
    //         //
    //         // This applies only when the supergraph.yaml file is present. Without it, we will
    //         // try composition each time we add a subgraph
    //         if let Some(supergraph_config) = self.supergraph_config.clone() {
    //             let subgraphs_from_config = supergraph_config.into_iter();
    //             if self.subgraphs.len() < subgraphs_from_config.len() {
    //                 return;
    //             }
    //         }
    //
    //         let composition_result = self.compose().await;
    //         if let Err(composition_err) = composition_result {
    //             eprintln!("{composition_err}");
    //         } else if composition_result.transpose().is_some() && !is_first_subgraph {
    //             eprintln!("successfully composed after adding the '{name}' subgraph");
    //         } else {
    //             return;
    //         }
    //     } else {
    //         eprintln!(
    //             "subgraph with name '{}' and url '{}' already exists",
    //             &name, &url
    //         );
    //     }
    // }

    // TODO: move this to a shared composer struct
    /// Updates a subgraph in the internal supergraph representation.
    async fn update_subgraph(
        &mut self,
        subgraph_name: SubgraphName,
        subgraph_config: ResolvedSubgraphConfig,
    ) {
        // TODO: use entries here?
        if let Some(prev_config) = self.supergraph.subgraphs.get_mut(&subgraph_name) {
            if *prev_config != subgraph_config {
                *prev_config = subgraph_config;
                let composition_result = self.compose().await;
                if let Err(composition_err) = composition_result {
                    eprintln!("{composition_err}");
                } else if composition_result.transpose().is_some() {
                    eprintln!(
                        "successfully composed after updating the '{subgraph_name}' subgraph"
                    );
                }
            }
        } else {
            eprintln!("subgraph with name '{}' does not exist", &subgraph_name);
        }
    }

    // TODO: Call this function only from the supergraph file watcher
    // /// Removes a subgraph from the internal subgraph representation.
    // async fn remove_subgraph(&mut self, subgraph_name: &SubgraphName) {
    //     let found = self
    //         .subgraphs
    //         .keys()
    //         .find(|(name, _)| name == subgraph_name)
    //         .cloned();
    //
    //     if let Some((name, url)) = found {
    //         self.subgraphs.remove(&(name.to_string(), url));
    //         let composition_result = self.compose().await;
    //         if let Err(composition_err) = composition_result {
    //             eprintln!("{composition_err}");
    //         } else if composition_result.transpose().is_some() {
    //             eprintln!("successfully composed after removing the '{name}' subgraph");
    //         }
    //     }
    // }

    /// Reruns composition, which triggers the router to reload.
    async fn compose(&mut self) -> CompositionResult {
        match self
            .compose_runner
            .run(&self.supergraph)
            .and_then(|maybe_new_schema| async {
                if maybe_new_schema.is_some() {
                    if let Some(runner) = self.router_runner.as_mut() {
                        if let Err(err) = runner.spawn().await {
                            return Err(err.to_string());
                        }
                    }
                }
                Ok(maybe_new_schema)
            })
            .await
        {
            Ok(res) => Ok(res),
            Err(e) => {
                if let Some(runner) = self.router_runner.as_mut() {
                    let _ = runner.kill().await.map_err(log_err_and_continue);
                }
                Err(e)
            }
        }
    }

    /// Handles a follower message by updating the internal subgraph representation if needed,
    /// and returns a [`LeaderMessageKind`] that can be sent over a socket or printed by the main session
    async fn handle_subgraph_message(&mut self, message: SubgraphUpdated) {
        eprintln!(
            "updating the schema for the '{}' subgraph in the session",
            message.subgraph_name
        );
        self.update_subgraph(message.subgraph_name, message.subgraph_config)
            .await;
    }
}
