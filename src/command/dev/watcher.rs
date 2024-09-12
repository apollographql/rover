use std::{collections::HashMap, path::PathBuf, str::FromStr, time::Duration};

use anyhow::{anyhow, Context};
use apollo_federation_types::build::SubgraphDefinition;
use camino::{Utf8Path, Utf8PathBuf};
use notify_debouncer_full::{
    new_debouncer,
    notify::{event::ModifyKind, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _},
    DebounceEventHandler, DebounceEventResult, Debouncer, FileIdMap,
};
use reqwest::Client;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::MissedTickBehavior::Delay;
use url::Url;

use rover_client::blocking::StudioClient;
use rover_client::operations::subgraph::fetch;
use rover_client::operations::subgraph::fetch::SubgraphFetchInput;
use rover_client::shared::GraphRef;
use rover_std::{errln, Fs};

use crate::{
    command::dev::{
        introspect::{IntrospectRunnerKind, UnknownIntrospectRunner},
        types::SubgraphKey,
    },
    RoverError, RoverErrorSuggestion, RoverResult,
};

#[derive(Debug)]
pub struct SubgraphSchemaWatcher {
    schema_watcher_kind: SubgraphSchemaWatcherKind,
    subgraph_key: SubgraphKey,
    subgraph_retries: u64,
    subgraph_retry_countdown: u64,
}

impl SubgraphSchemaWatcher {
    pub fn new_from_file_path<P>(
        subgraph_key: SubgraphKey,
        path: P,
        subgraph_retries: u64,
    ) -> RoverResult<Self>
    where
        P: AsRef<Utf8Path>,
    {
        Ok(Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::File(path.as_ref().to_path_buf()),
            subgraph_key,
            subgraph_retries,
            subgraph_retry_countdown: 0,
        })
    }

    pub fn new_from_url(
        subgraph_key: SubgraphKey,
        client: Client,
        polling_interval: u64,
        headers: Option<HashMap<String, String>>,
        subgraph_retries: u64,
        subgraph_url: Url,
    ) -> RoverResult<Self> {
        let headers = headers.map(|header_map| header_map.into_iter().collect());
        let introspect_runner = IntrospectRunnerKind::Unknown(UnknownIntrospectRunner::new(
            subgraph_url,
            client,
            headers,
        ));
        Self::new_from_introspect_runner(
            subgraph_key,
            introspect_runner,
            polling_interval,
            subgraph_retries,
        )
    }

    pub fn new_from_sdl(
        subgraph_key: SubgraphKey,
        sdl: String,
        subgraph_retries: u64,
    ) -> RoverResult<Self> {
        Ok(Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::Once(sdl),
            subgraph_key,
            subgraph_retries,
            subgraph_retry_countdown: 0,
        })
    }

    pub async fn new_from_graph_ref(
        graph_ref: &str,
        graphos_subgraph_name: String,
        routing_url: Option<Url>,
        yaml_subgraph_name: String,
        client: &StudioClient,
        subgraph_retries: u64,
    ) -> RoverResult<Self> {
        // given a graph_ref and subgraph, run subgraph fetch to
        // obtain SDL and add it to subgraph_definition.
        let response = fetch::run(
            SubgraphFetchInput {
                graph_ref: GraphRef::from_str(graph_ref)?,
                subgraph_name: graphos_subgraph_name.clone(),
            },
            client,
        )
        .await
        .map_err(RoverError::from)?;
        let routing_url = match (routing_url, response.sdl.r#type) {
            (Some(routing_url), _) => routing_url,
            (
                None,
                rover_client::shared::SdlType::Subgraph {
                    routing_url: Some(graph_registry_routing_url),
                },
            ) => graph_registry_routing_url.parse().context(format!(
                "Could not parse graph registry routing url {}",
                graph_registry_routing_url
            ))?,
            (None, _) => {
                return Err(RoverError::new(anyhow!(
                    "Could not find routing URL in GraphOS for subgraph {graphos_subgraph_name}"
                ))
                .with_suggestion(RoverErrorSuggestion::AddRoutingUrlToSupergraphYaml)
                .with_suggestion(
                    RoverErrorSuggestion::PublishSubgraphWithRoutingUrl {
                        subgraph_name: yaml_subgraph_name,
                        graph_ref: graph_ref.to_string(),
                    },
                ));
            }
        };
        Self::new_from_sdl(
            (yaml_subgraph_name, routing_url),
            response.sdl.contents,
            subgraph_retries,
        )
    }

    pub fn new_from_introspect_runner(
        subgraph_key: SubgraphKey,
        introspect_runner: IntrospectRunnerKind,
        polling_interval: u64,
        subgraph_retries: u64,
    ) -> RoverResult<Self> {
        Ok(Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::Introspect(
                introspect_runner,
                polling_interval,
            ),
            subgraph_key,
            subgraph_retries,
            subgraph_retry_countdown: 0,
        })
    }

    pub async fn get_subgraph_definition_and_maybe_new_runner(
        &self,
        retry_period: Option<Duration>,
    ) -> RoverResult<(SubgraphDefinition, Option<SubgraphSchemaWatcherKind>)> {
        let (name, url) = self.subgraph_key.clone();
        let (sdl, refresher) = match &self.schema_watcher_kind {
            SubgraphSchemaWatcherKind::Introspect(introspect_runner_kind, polling_interval) => {
                match introspect_runner_kind {
                    IntrospectRunnerKind::Graph(graph_runner) => {
                        let sdl = graph_runner.run().await?;
                        (sdl, None)
                    }
                    IntrospectRunnerKind::Subgraph(subgraph_runner) => {
                        let sdl = subgraph_runner.run().await?;
                        (sdl, None)
                    }
                    IntrospectRunnerKind::Unknown(unknown_runner) => {
                        let (sdl, specific_runner) = unknown_runner.run(retry_period).await?;
                        (
                            sdl,
                            Some(SubgraphSchemaWatcherKind::Introspect(
                                specific_runner,
                                *polling_interval,
                            )),
                        )
                    }
                }
            }
            SubgraphSchemaWatcherKind::File(file_path) => {
                let sdl = Fs::read_file(file_path)?;
                (sdl, None)
            }
            SubgraphSchemaWatcherKind::Once(sdl) => (sdl.clone(), None),
        };

        let subgraph_definition = SubgraphDefinition::new(name, url, sdl);

        Ok((subgraph_definition, refresher))
    }

    async fn update_subgraph(
        &mut self,
        last_message: Option<&String>,
        retry_period: Option<Duration>,
    ) -> RoverResult<Option<String>> {
        let maybe_update_message = match self
            .get_subgraph_definition_and_maybe_new_runner(retry_period)
            .await
        {
            Ok((subgraph_definition, maybe_new_refresher)) => {
                if let Some(new_refresher) = maybe_new_refresher {
                    self.set_schema_refresher(new_refresher);
                }
                match last_message {
                    Some(last_message) => {
                        if &subgraph_definition.sdl != last_message {
                            if self.subgraph_retry_countdown < self.subgraph_retries {
                                eprintln!(
                                    "subgraph connectivity restored for {}",
                                    self.subgraph_key.0
                                )
                            }
                            todo!("update_subgraph");
                        }
                    }
                    None => {
                        todo!("update_subgraph");
                    }
                }
                self.subgraph_retry_countdown = self.subgraph_retries;
                Some(subgraph_definition.sdl)
            }
            Err(e) => {
                // `subgraph-retries` can be set by the user away from the default value of 0,
                // this defaults to Rover's current behaviour.
                //
                // If a user does set this value to a non-zero one, and we get a non-retryable error
                // from one of our subgraphs, we'll retain the old schema we had and continue
                // operation. This will happen until the countdown hits 0 at which point the
                // subgraph will be disconnected from the supergraph.
                //
                // Every time we successfully communicate with the subgraph we set the countdown
                // back to the maximum value.
                //
                if self.subgraph_retry_countdown > 0 {
                    self.subgraph_retry_countdown -= 1;
                    errln!("error detected communicating with subgraph '{}', schema changes will not be reflected.\nWill retry but subgraph logs should be inspected", &self.subgraph_key.0);
                    errln!("{:}", e);
                    Some(e.to_string())
                } else {
                    eprintln!(
                        "retries exhausted for subgraph {}. To add more run `rover dev` with the --subgraph-retries flag.",
                        &self.subgraph_key.0,
                    );
                    todo!("update_subgraph");
                    None
                }
            }
        };

        Ok(maybe_update_message)
    }

    /// Start checking for subgraph updates and sending them to the main process.
    ///
    /// This function will block forever for `SubgraphSchemaWatcherKind` that poll for changes—so it
    /// should be started in a separate thread.
    pub async fn watch_subgraph_for_changes(
        &mut self,
        retry_period: Option<Duration>,
    ) -> RoverResult<()> {
        let mut last_message = None;
        match self.schema_watcher_kind.clone() {
            SubgraphSchemaWatcherKind::Introspect(introspect_runner_kind, polling_interval) => {
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
                let mut interval = tokio::time::interval(Duration::from_secs(polling_interval));
                interval.set_missed_tick_behavior(Delay);
                loop {
                    last_message = self
                        .update_subgraph(last_message.as_ref(), retry_period)
                        .await?;
                    interval.tick().await;
                }
            }
            SubgraphSchemaWatcherKind::File(path) => {
                // populate the schema for the first time (last_message is always None to start)
                last_message = self
                    .update_subgraph(last_message.as_ref(), retry_period)
                    .await?;

                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

                let watch_path = path.clone();

                Fs::watch_file(watch_path, tx);

                while let Some(res) = rx.recv().await {
                    match res {
                        Ok(()) => (),
                        Err(err) => return Err(anyhow::Error::from(err).into()),
                    }
                    last_message = self
                        .update_subgraph(last_message.as_ref(), retry_period)
                        .await?;
                }
            }
            SubgraphSchemaWatcherKind::Once(_) => {
                self.update_subgraph(None, retry_period).await?;
            }
        }
        Ok(())
    }

    pub fn set_schema_refresher(&mut self, new_refresher: SubgraphSchemaWatcherKind) {
        self.schema_watcher_kind = new_refresher;
    }

    pub fn get_name(&self) -> String {
        self.subgraph_key.0.to_string()
    }
}

#[derive(Debug, Clone)]
pub enum SubgraphSchemaWatcherKind {
    /// Poll an endpoint via introspection
    Introspect(IntrospectRunnerKind, u64),
    /// Watch a file on disk
    File(Utf8PathBuf),
    /// Don't ever update, schema is only pulled once
    Once(String),
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
