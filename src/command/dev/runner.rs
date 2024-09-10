use std::{collections::HashMap, path::PathBuf, time::Duration};

use anyhow::anyhow;
use apollo_federation_types::config::{SchemaSource, SupergraphConfig};
use notify_debouncer_full::{
    new_debouncer,
    notify::{event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _},
    DebounceEventHandler, DebounceEventResult, Debouncer, FileIdMap,
};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::{
    command::dev::{
        compose::ComposeRunner,
        router::{RouterConfigHandler, RouterRunner},
    },
    options::{PluginOpts, ProfileOpt},
    utils::{client::StudioClientConfig, supergraph_config::get_supergraph_config},
    RoverError, RoverResult,
};

use super::SupergraphOpts;

pub struct Runner {
    client_config: StudioClientConfig,
    compose_runner: ComposeRunner,
    router_runner: RouterRunner,
    router_config_handler: RouterConfigHandler,
    supergraph_opts: SupergraphOpts,
    watchers: HashMap<WatchType, Watcher>,
}

impl Runner {
    pub fn new(
        client_config: &StudioClientConfig,
        plugin_opts: PluginOpts,
        router_config_handler: RouterConfigHandler,
        supergraph_opts: &SupergraphOpts,
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
            client_config: client_config.clone(),
            compose_runner,
            router_runner,
            router_config_handler,
            supergraph_opts: supergraph_opts.clone(),
            watchers: HashMap::new(),
        }
    }

    pub async fn run(&mut self, profile: &ProfileOpt) -> RoverResult<()> {
        tracing::info!("initializing main `rover dev process`");

        let supergraph_config = self.load_supergraph_config(profile).await?;

        // install plugins before proceeding
        self.router_runner.maybe_install_router().await?;
        self.compose_runner
            .maybe_install_supergraph(supergraph_config.get_federation_version().unwrap())
            .await?;
        self.router_config_handler.clone().start()?;

        // Start supergraph watcher.
        self.watchers.insert(
            WatchType::Supergraph,
            Watcher::new(
                self.supergraph_opts
                    .supergraph_config_path
                    .as_ref()
                    .unwrap()
                    .to_path_buf()
                    .unwrap()
                    .into(),
                WatchType::Supergraph,
            )
            .await,
        );

        // Start subgraph watchers.
        // TODO: need to use or refactor all of this from schema.rs/watcher.rs
        for (name, subgraph_config) in supergraph_config.into_iter() {
            match subgraph_config.schema {
                SchemaSource::File { file } => {
                    self.watchers.insert(
                        WatchType::Subgraph(name.clone()),
                        Watcher::new(PathBuf::from(file.as_std_path()), WatchType::Subgraph(name))
                            .await,
                    );
                }
                SchemaSource::SubgraphIntrospection {
                    subgraph_url,
                    introspection_headers,
                } => todo!(),
                SchemaSource::Sdl { sdl } => todo!(),
                SchemaSource::Subgraph {
                    graphref,
                    subgraph: graphos_subgraph_name,
                } => todo!(),
            };
        }

        loop {
            let futs: Vec<_> = self
                .watchers
                .iter_mut()
                .map(|(_, w)| Box::pin(w.recv()))
                .collect();

            let (res, _, _) = futures::future::select_all(futs.into_iter()).await;
            match res {
                Some(WatchType::Supergraph) => {
                    // TODO:
                    // 1. Re-parse supergraph config
                    // 2. Update subgraph map with new values
                    // 3. Re-compose
                    eprintln!("supergraph update");
                }
                Some(WatchType::Subgraph(subgraph)) => {
                    // TODO: read new subgraph config and recompose.
                    eprintln!("subgraph update: {subgraph}");
                }
                None => {
                    eprintln!("Unknown WatchType");
                    break;
                }
            }
        }

        Ok(())
    }

    async fn load_supergraph_config(&self, profile: &ProfileOpt) -> RoverResult<SupergraphConfig> {
        get_supergraph_config(
            &self.supergraph_opts.graph_ref,
            &self.supergraph_opts.supergraph_config_path,
            self.supergraph_opts.federation_version.as_ref(),
            self.client_config.clone(),
            profile,
            false,
        )
        .await
        .map_err(|_| RoverError::new(anyhow!("TODO: get actual error")))?
        .ok_or_else(|| RoverError::new(anyhow!("supergraph config None?")))
    }

    pub async fn shutdown(mut self) -> RoverResult<()> {
        self.router_runner
            .kill()
            .await
            .map_err(|_| RoverError::new(anyhow!("could not shut down router")))?;
        Ok(())
    }
}

pub struct Watcher {
    watch_type: WatchType,
    debouncer: Debouncer<RecommendedWatcher, FileIdMap>,
    rx: Receiver<()>,
}

impl Watcher {
    pub async fn new(path: PathBuf, watch_type: WatchType) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(5);

        // TODO: is storing this the only way we can survive a drop?
        let mut debouncer = new_debouncer(Duration::from_secs(1), None, SenderWrapper(tx)).unwrap();
        debouncer
            .watcher()
            .watch(&path, RecursiveMode::NonRecursive)
            .unwrap();

        Self {
            watch_type,
            debouncer,
            rx,
        }
    }

    pub async fn recv(&mut self) -> Option<WatchType> {
        self.rx.recv().await.map(|_| self.watch_type.clone())
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub enum WatchType {
    Supergraph,
    Subgraph(String),
}

struct SenderWrapper(Sender<()>);

impl DebounceEventHandler for SenderWrapper {
    fn handle_event(&mut self, res: DebounceEventResult) {
        for event in res.unwrap() {
            if let EventKind::Modify(ModifyKind::Data(..)) = event.kind {
                self.0.try_send(()).ok(); // TODO: handle error instead of using ok().
            }
        }
    }
}
