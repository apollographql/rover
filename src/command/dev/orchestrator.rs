use crate::federation::{Event, SubgraphSchemaWatcherKind, Watcher};
use crate::{
    command::dev::{
        do_dev::log_err_and_continue,
        router::{RouterConfigHandler, RouterRunner},
    },
    options::PluginOpts,
    utils::client::StudioClientConfig,
    RoverError, RoverErrorSuggestion, RoverResult,
};
use anyhow::{anyhow, Error};
use camino::Utf8PathBuf;
use rover_client::RoverClientError;
use rover_std::infoln;
use std::io::Write;
use std::{fmt::Debug, fs, net::TcpListener};
use tracing::info;

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
        watcher: Watcher,
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

    /// Listen for incoming subgraph updates and re-compose the supergraph
    pub(crate) async fn run(self) -> RoverResult<()> {
        // TODO: notify on each watcher startup?
        // TODO: make an easier way to get at subgraphs... `into_iter()` is silly in most places
        let mut messages = self.watcher.watch().await;
        let mut router_runner = self.router_runner;
        while let Some(event) = messages.recv().await {
            match event {
                Event::SubgraphAdded { .. } | Event::SubgraphUpdated { .. } => (), // TODO: show a spinner or something when composition starts?
                Event::SubgraphRemoved { subgraph_name } => {
                    infoln!("Removed subgraph {subgraph_name}")
                }
                Event::StartedWatchingSubgraph(kind) => match kind {
                    SubgraphSchemaWatcherKind::File(path) => {
                        infoln!("Watching {} for changes", path.as_std_path().display());
                    }
                    SubgraphSchemaWatcherKind::Introspect(
                        introspect_runner_kind,
                        polling_interval,
                    ) => {
                        let endpoint = introspect_runner_kind.endpoint();
                        eprintln!(
                            "polling {} every {} {}",
                            &endpoint,
                            polling_interval,
                            match polling_interval {
                                1 => "second",
                                _ => "seconds",
                            }
                        );
                    }
                },
                Event::CompositionSucceeded {
                    federation_version,
                    subgraph_name,
                    output,
                } => {
                    respawn_router(
                        &self.supergraph_schema_path,
                        &output.supergraph_sdl,
                        router_runner.as_mut(),
                    )
                    .await?;
                    if let Some(subgraph_name) = subgraph_name {
                        eprintln!(
                            "successfully composed with version {federation_version} after updating the '{subgraph_name}' subgraph"
                        );
                    } else {
                        eprintln!("successfully composed with version {federation_version}");
                    }
                }
                Event::CompositionFailed { err, .. } => {
                    if let Some(runner) = router_runner.as_mut() {
                        let _ = runner.kill().await.map_err(log_err_and_continue);
                    }
                    eprintln!("{err}");
                }
                Event::CompositionErrors { errors, .. } => {
                    let rover_error =
                        RoverError::from(RoverClientError::BuildErrors { source: errors });
                    if let Some(runner) = router_runner.as_mut() {
                        let _ = runner.kill().await.map_err(log_err_and_continue);
                    }
                    eprintln!("{rover_error}");
                }
            }
        }
        Ok(())
    }
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
