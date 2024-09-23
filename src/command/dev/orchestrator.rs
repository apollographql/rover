use crate::federation::{Event, Watcher};
use crate::{
    command::dev::{
        do_dev::log_err_and_continue,
        router::{RouterConfigHandler, RouterRunner},
        OVERRIDE_DEV_COMPOSITION_VERSION,
    },
    options::PluginOpts,
    utils::client::StudioClientConfig,
    RoverError, RoverErrorSuggestion, RoverResult,
};
use anyhow::{anyhow, Error};
use apollo_federation_types::config::{FederationVersion, SchemaSource};
use camino::Utf8PathBuf;
use rover_client::RoverClientError;
use rover_std::infoln;
use std::io::Write;
use std::str::FromStr;
use std::{fmt::Debug, fs, net::TcpListener};
use tracing::{info, warn};

/// The top-level runner which handles router, recomposition of supergraphs, and wrangling the various `SubgraphWatcher`s
#[derive(Debug)]
pub(crate) struct Orchestrator {
    router_runner: Option<RouterRunner>,
    watcher: Watcher,
    /// Where the router is watching a `supergraph.graphql`
    supergraph_schema_path: Utf8PathBuf,
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
        plugin_opts: PluginOpts,
        mut watcher: Watcher,
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
        let _ = fs::remove_file(&raw_socket_name);

        if TcpListener::bind(router_socket_addr).is_err() {
            let mut err =
                RoverError::new(anyhow!("You cannot bind the router to '{}' because that address is already in use by another process on this machine.", &router_socket_addr));
            err.set_suggestion(RoverErrorSuggestion::Adhoc(
                format!("Try setting a different port for the router to bind to with the `--supergraph-port` argument, or shut down the process bound to '{}'.", &router_socket_addr)
            ));
            return Err(err);
        }

        // TODO: when the user changes federation version in supergraph config, should it update?
        if let Some(version_from_env) = Self::get_federation_version_from_env() {
            watcher.composer.supergraph_config.federation_version = version_from_env;
        };

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
        let supergraph_schema_path = router_config_handler.get_supergraph_schema_path();
        router_config_handler.start()?;

        let orchestrator = Self {
            watcher,
            router_runner: Some(router_runner),
            supergraph_schema_path,
        };
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
    pub(crate) async fn run(self) -> RoverResult<()> {
        // TODO: update this when we add subgraph add/remove events
        let num_subgraphs = self.watcher.composer.supergraph_config.subgraphs.len();
        // TODO: notify on each watcher startup?
        // TODO: make an easier way to get at subgraphs... `into_iter()` is silly in most places
        for (_, subgraph) in self.watcher.supergraph_config.clone().into_iter() {
            if let SchemaSource::File { file: path } = subgraph.schema {
                infoln!("Watching {} for changes", path.as_std_path().display());
            }
        }
        let mut messages = self.watcher.watch().await;
        let mut router_runner = self.router_runner;
        while let Some(event) = messages.recv().await {
            match event {
                Event::SubgraphUpdated { subgraph_name } => {
                    eprintln!(
                        "updating the schema for the '{}' subgraph in the session",
                        subgraph_name
                    );
                }
                Event::ComposedAfterSubgraphUpdated {
                    subgraph_name,
                    output,
                } => {
                    respawn_router(
                        &self.supergraph_schema_path,
                        &output.supergraph_sdl,
                        router_runner.as_mut(),
                    )
                    .await?;
                    eprintln!(
                        "successfully composed after updating the '{subgraph_name}' subgraph"
                    );
                }
                Event::InitialComposition(output) => {
                    respawn_router(
                        &self.supergraph_schema_path,
                        &output.supergraph_sdl,
                        router_runner.as_mut(),
                    )
                    .await?;
                }
                Event::CompositionFailed(rover_error) => {
                    if let Some(runner) = router_runner.as_mut() {
                        let _ = runner.kill().await.map_err(log_err_and_continue);
                    }
                    eprintln!("{rover_error}");
                }
                Event::CompositionErrors(build_errors) => {
                    let rover_error = RoverError::from(RoverClientError::BuildErrors {
                        source: build_errors.clone(),
                        num_subgraphs,
                    });
                    if let Some(runner) = router_runner.as_mut() {
                        let _ = runner.kill().await.map_err(log_err_and_continue);
                    }
                    eprintln!("{rover_error}");
                }
            }
        }
        Ok(())
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
}

async fn respawn_router(
    supergraph_schema_path: &Utf8PathBuf,
    supergraph_sdl: &str,
    router_runner: Option<&mut RouterRunner>,
) -> RoverResult<()> {
    update_supergraph_schema(supergraph_schema_path, supergraph_sdl)?;
    if let Some(runner) = router_runner {
        runner.maybe_spawn().await?
    }
    Ok(())
}

fn update_supergraph_schema(path: &Utf8PathBuf, sdl: &str) -> RoverResult<()> {
    info!("composition succeeded, updating the supergraph schema...");
    let context = format!("could not write SDL to {}", path);
    match fs::File::create(path) {
        Ok(mut opened_file) => {
            if let Err(e) = opened_file.write_all(sdl.as_bytes()) {
                Err(RoverError::new(
                    Error::new(e)
                        .context("could not write bytes")
                        .context(context),
                ))
            } else if let Err(e) = opened_file.flush() {
                Err(RoverError::new(
                    Error::new(e)
                        .context("could not flush file")
                        .context(context),
                ))
            } else {
                info!("wrote updated supergraph schema to {}", path);
                Ok(())
            }
        }
        Err(e) => Err(RoverError::new(Error::new(e).context(context))),
    }
}
