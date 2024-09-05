use std::{borrow::BorrowMut, collections::HashMap, fmt::Debug};

use anyhow::{anyhow, Context};
use apollo_federation_types::config::SupergraphConfig;
use futures::{Future, TryFutureExt};
use rover_std::{infoln, warnln, Fs};

use crate::{
    command::dev::{
        compose::ComposeRunner,
        router::{RouterConfigHandler, RouterRunner},
    },
    options::PluginOpts,
    utils::client::StudioClientConfig,
    RoverError, RoverResult,
};

#[derive(Debug)]
pub struct Runner {
    compose_runner: ComposeRunner,
    router_runner: RouterRunner,
    router_config_handler: RouterConfigHandler,
}

impl Runner {
    pub fn new(
        plugin_opts: PluginOpts,
        client_config: &StudioClientConfig,
        router_config_handler: RouterConfigHandler,
    ) -> Self {
        // Create a [`ComposeRunner`] that will be in charge of composing our supergraph
        let compose_runner = ComposeRunner::new(
            plugin_opts.clone(),
            None, // TODO: need to pass this.
            client_config.clone(),
            router_config_handler.get_supergraph_schema_path(),
        );

        // Create a [`RouterRunner`] that will be in charge of running the router
        let router_runner = RouterRunner::new(
            router_config_handler.get_supergraph_schema_path(),
            router_config_handler.get_router_config_path(),
            plugin_opts.clone(),
            router_config_handler.get_router_address(),
            router_config_handler.get_router_listen_path(),
            None, // TODO: need to pass this.
            client_config.clone(),
            None, // TODO: need to pass this.
        );

        Self {
            compose_runner,
            router_runner,
            router_config_handler,
        }
    }

    pub async fn run(&mut self, mut supergraph_config: SupergraphConfig) -> RoverResult<()> {
        tracing::info!("initializing main `rover dev process`");
        warnln!(
            "Do not run this command in production! It is intended for local development only."
        );
        infoln!("Starting main `rover dev` process");

        // Install the necessary plugins if they're not already installed (supergraph binary and
        // the router binary)
        self.install_plugins(&supergraph_config).await?;

        // We do an initial composition check to ensure that, before starting the router, we have
        // something that the router can run
        self.compose_runner
            .run(supergraph_config.borrow_mut())
            .map_err(|e| anyhow!(e))
            .await?
            .ok_or(RoverError::from(anyhow!(
                "failed to compose: no composition result"
            )))?;

        // Start the watcher for the router config to hot reload the router when necessary
        self.router_config_handler.clone().start()?;

        // Start the router
        self.router_runner.spawn().await?;

        Ok(())
    }

    /// Install the necessary plugins for composition (the supergraph binary) and the router (the
    /// router binary) if they're not already installed
    async fn install_plugins(&mut self, supergraph_config: &SupergraphConfig) -> RoverResult<()> {
        // install plugins before proceeding
        self.router_runner.maybe_install_router().await?;
        self.compose_runner
            .maybe_install_supergraph(supergraph_config.get_federation_version().unwrap())
            .await?;

        RoverResult::Ok(())
    }

    pub async fn watch_supergraph_config() -> RoverResult<()> {
        // TODO: set up supergraph watcher.
        // TODO: compose subgraph watchers from supergraph config.
        // let path = &self
        //     .opts
        //     .supergraph_opts
        //     .supergraph_config_path
        //     .as_ref()
        //     .unwrap()
        //     .to_path_buf()
        //     .unwrap();

        // let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        // Fs::watch_file(path, tx);
        // tokio::spawn(async move {
        //     loop {
        //         let _ = rx.recv().await.unwrap();
        //         rover_std::infoln!("supergraph config updated");
        //     }
        // });
        //
        Ok(())
    }
}
