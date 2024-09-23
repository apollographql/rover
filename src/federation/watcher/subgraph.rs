use apollo_federation_types::config::{SchemaSource, SupergraphConfig};
use camino::{Utf8Path, Utf8PathBuf};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tokio::sync::mpsc::Sender;
use tokio::time::MissedTickBehavior::Delay;
use url::Url;

use rover_std::{errln, Fs};

use super::introspect::{IntrospectRunnerKind, UnknownIntrospectRunner};
use crate::utils::client::StudioClientConfig;
use crate::RoverResult;

/// Watches a subgraph for schema updates
#[derive(Debug)]
pub(crate) struct Watcher {
    schema_watcher_kind: SubgraphSchemaWatcherKind,
    subgraph_name: String,
    message_sender: Sender<Updated>,
    retry_period: Option<Duration>,
}

impl Watcher {
    pub fn new_from_file_path<P>(
        subgraph_name: String,
        path: P,
        message_sender: Sender<Updated>,
        retry_period: Option<Duration>,
    ) -> Self
    where
        P: AsRef<Utf8Path>,
    {
        Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::File(path.as_ref().to_path_buf()),
            subgraph_name,
            message_sender,
            retry_period,
        }
    }

    pub fn new_from_url(
        subgraph_name: String,
        client: Client,
        message_sender: Sender<Updated>,
        polling_interval: u64,
        headers: Option<HashMap<String, String>>,
        subgraph_url: Url,
        retry_period: Option<Duration>,
    ) -> Self {
        let headers = headers.map(|header_map| header_map.into_iter().collect());
        let introspect_runner = IntrospectRunnerKind::Unknown(UnknownIntrospectRunner::new(
            subgraph_url,
            client,
            headers,
        ));
        Self::new_from_introspect_runner(
            subgraph_name,
            introspect_runner,
            message_sender,
            polling_interval,
            retry_period,
        )
    }

    pub fn new_from_introspect_runner(
        subgraph_name: String,
        introspect_runner: IntrospectRunnerKind,
        message_sender: Sender<Updated>,
        polling_interval: u64,
        retry_period: Option<Duration>,
    ) -> Self {
        Self {
            schema_watcher_kind: SubgraphSchemaWatcherKind::Introspect(
                introspect_runner,
                polling_interval,
            ),
            subgraph_name,
            message_sender,
            retry_period,
        }
    }

    pub async fn get_subgraph_sdl_and_maybe_new_runner(
        &self,
    ) -> RoverResult<(String, Option<SubgraphSchemaWatcherKind>)> {
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
                        let (sdl, specific_runner) = unknown_runner.run(self.retry_period).await?;
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
        };

        Ok((sdl, refresher))
    }

    async fn update_subgraph(
        &mut self,
        last_message: Option<&String>,
    ) -> RoverResult<Option<String>> {
        let maybe_update_message = match self.get_subgraph_sdl_and_maybe_new_runner().await {
            Ok((sdl, maybe_new_refresher)) => {
                if let Some(new_refresher) = maybe_new_refresher {
                    self.set_schema_refresher(new_refresher);
                }
                if let Some(last_message) = last_message {
                    if &sdl != last_message {
                        eprintln!("subgraph connectivity restored for {}", self.subgraph_name);
                        self.message_sender
                            .send(Updated {
                                subgraph_name: self.subgraph_name.clone(),
                                new_sdl: sdl.clone(),
                            })
                            .await?;
                    }
                }
                Some(sdl) // TODO: why do we need to return this?
            }
            Err(e) => {
                // TODO: send an error event for this
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
                errln!("error detected communicating with subgraph '{}', schema changes will not be reflected.\nWill retry but subgraph logs should be inspected", &self.subgraph_name);
                errln!("{:}", e);
                Some(e.to_string())
            }
        };

        Ok(maybe_update_message)
    }

    /// Start checking for subgraph updates and sending them to the main process.
    ///
    /// This function will block forever for `SubgraphSchemaWatcherKind` that poll for changes—so it
    /// should be started in a separate thread.
    pub async fn watch_subgraph_for_changes(mut self) -> RoverResult<()> {
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
                    last_message = self.update_subgraph(last_message.as_ref()).await?;
                    interval.tick().await;
                }
            }
            SubgraphSchemaWatcherKind::File(path) => {
                // populate the schema for the first time (last_message is always None to start)
                last_message = self.update_subgraph(last_message.as_ref()).await?;

                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

                let watch_path = path.clone();

                Fs::watch_file(watch_path, tx);

                while let Some(res) = rx.recv().await {
                    match res {
                        Ok(()) => (),
                        Err(err) => return Err(anyhow::Error::from(err).into()),
                    }
                    last_message = self.update_subgraph(last_message.as_ref()).await?;
                }
            }
        }
        Ok(())
    }

    pub fn set_schema_refresher(&mut self, new_refresher: SubgraphSchemaWatcherKind) {
        self.schema_watcher_kind = new_refresher;
    }
}

pub(super) async fn get_watchers(
    client_config: &StudioClientConfig,
    supergraph_config: SupergraphConfig,
    messenger: Sender<Updated>,
    polling_interval: u64,
) -> RoverResult<HashMap<String, Watcher>> {
    let client = client_config
        .get_builder()
        .with_timeout(Duration::from_secs(5))
        .build()?;

    let watchers = supergraph_config
        .into_iter()
        .filter_map(|(subgraph_name, subgraph_config)| {
            match subgraph_config.schema {
                SchemaSource::File { file } => Some(Watcher::new_from_file_path(
                    subgraph_name.clone(),
                    file,
                    messenger.clone(),
                    client_config.retry_period,
                )),
                SchemaSource::SubgraphIntrospection {
                    subgraph_url,
                    introspection_headers,
                } => Some(Watcher::new_from_url(
                    subgraph_name.clone(),
                    client.clone(),
                    messenger.clone(),
                    polling_interval,
                    introspection_headers,
                    subgraph_url,
                    client_config.retry_period,
                )),
                SchemaSource::Sdl { .. } | SchemaSource::Subgraph { .. } => {
                    // We don't watch these
                    None
                }
            }
            .map(|watcher| (subgraph_name, watcher))
        })
        .collect();
    Ok(watchers)
}

#[derive(Debug, Clone)]
pub(crate) enum SubgraphSchemaWatcherKind {
    /// Poll an endpoint via introspection
    Introspect(IntrospectRunnerKind, u64),
    /// Watch a file on disk
    File(Utf8PathBuf),
}

/// These are the messages sent from `SubgraphWatcher` to `Orchestrator`
#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct Updated {
    pub(crate) subgraph_name: String,
    pub(crate) new_sdl: String,
}
