use std::fmt::Debug;

use apollo_federation_types::config::SupergraphConfig;
use futures::TryFutureExt;
use rover_std::{warnln, Fs};

use crate::{
    command::dev::{
        compose::ComposeRunner,
        router::{RouterConfigHandler, RouterRunner},
        types::CompositionResult,
    },
    options::PluginOpts,
    utils::client::StudioClientConfig,
    RoverResult,
};

#[derive(Debug)]
pub struct Runner {
    compose_runner: ComposeRunner,
    router_runner: RouterRunner,
}

impl Runner {
    pub async fn new(
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
        }
    }

    pub async fn start(supergraph_config: SupergraphConfig) -> RoverResult<()> {
        tracing::info!("initializing main `rover dev process`");
        warnln!(
            "Do not run this command in production! It is intended for local development only."
        );

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

        Ok(())
    }

    pub async fn shutdown(&mut self) {
        self.router_runner.kill().await.unwrap();
        std::process::exit(1) // TODO: maybe return a result instead?
    }

    pub async fn watch_supergraph_config() -> RoverResult<()> {
        Ok(())
    }

    async fn compose(&mut self, mut supergraph_config: SupergraphConfig) -> CompositionResult {
        match self
            .compose_runner
            .run(&mut supergraph_config)
            .and_then(|maybe_new_schema| async {
                if maybe_new_schema.is_some() {
                    if let Err(err) = self.router_runner.spawn().await {
                        return Err(err.to_string());
                    }
                }
                Ok(maybe_new_schema)
            })
            .await
        {
            Ok(res) => Ok(res),
            Err(e) => {
                self.router_runner.kill().await.unwrap();
                Err(e)
            }
        }
    }
}
