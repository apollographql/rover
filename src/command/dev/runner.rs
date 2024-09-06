use std::{collections::HashMap, path::PathBuf, time::Duration};

use apollo_federation_types::config::SupergraphConfig;
use notify_debouncer_full::{
    new_debouncer,
    notify::{event::ModifyKind, EventKind, RecursiveMode, Watcher as _},
    DebounceEventHandler, DebounceEventResult,
};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::{
    command::dev::{
        compose::ComposeRunner,
        router::{RouterConfigHandler, RouterRunner},
    },
    options::PluginOpts,
    utils::client::StudioClientConfig,
    RoverResult,
};

pub struct Runner {
    compose_runner: ComposeRunner,
    router_runner: RouterRunner,
    router_config_handler: RouterConfigHandler,
    watchers: HashMap<WatchType, Watcher>,
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
            watchers: HashMap::new(),
        }
    }

    pub async fn run(&mut self, supergraph_config: SupergraphConfig) -> RoverResult<()> {
        tracing::info!("initializing main `rover dev process`");

        // install plugins before proceeding
        self.router_runner.maybe_install_router().await?;
        self.compose_runner
            .maybe_install_supergraph(supergraph_config.get_federation_version().unwrap())
            .await?;
        self.router_config_handler.clone().start()?;

        // Insert supergraph watcher.
        self.watchers.insert(
            WatchType::Supergraph,
            Watcher::new(
                PathBuf::from("examples/supergraph-demo/supergraph.yaml"),
                WatchType::Supergraph,
            )
            .await,
        );

        // TODO: insert subgraph watchers.
        // subgraph_watchers: futures::future::join_all(subgraphs.into_iter().map(
        //     |(key, path)| async {
        //         (
        //             key.clone(),
        //             Watcher::new(path,  WatchType::Subgraph(key)).await,
        //         )
        //     },
        // ))
        // .await
        // .into_iter()
        // .collect(),

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
                    eprintln!("supergraph update: {subgraph}");
                }
                None => {
                    eprintln!("Unknown WatchType");
                    break;
                }
            }
        }

        Ok(())
    }

    pub async fn shutdown(mut self) {
        let _ = self.watchers.iter_mut().map(|(_, w)| w.close());
        self.router_runner.kill().await.unwrap();
        std::process::exit(1) // TODO: maybe return a result instead?
    }
}

pub struct Watcher {
    rx: Receiver<DebounceEventResult>,
    watch_type: WatchType,
}

impl Watcher {
    pub async fn new(path: PathBuf, watch_type: WatchType) -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(5);

        // TODO: could get dropped, so might need to be inside below thread.
        new_debouncer(Duration::from_secs(1), None, SenderWrapper(tx))
            .unwrap()
            .watcher()
            .watch(&path, RecursiveMode::NonRecursive)
            .unwrap();

        Self { rx, watch_type }
    }

    pub async fn recv(&mut self) -> Option<WatchType> {
        self.rx.recv().await.map(|_| self.watch_type.clone())
    }

    pub fn close(&mut self) {
        self.rx.close();
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub enum WatchType {
    Supergraph,
    Subgraph(String),
}

struct SenderWrapper<T>(Sender<T>);

impl DebounceEventHandler for SenderWrapper<DebounceEventResult> {
    fn handle_event(&mut self, res: DebounceEventResult) {
        for event in res.unwrap() {
            if let EventKind::Modify(ModifyKind::Data(..)) = event.kind {
                // self.0.try_send(event).ok(); // TODO: handle error instead of using ok().
            }
        }
    }
}
