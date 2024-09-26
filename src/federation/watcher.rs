use crate::federation::supergraph_config::{
    resolve_subgraph, resolve_supergraph_config, HybridSupergraphConfig,
};
use crate::federation::watcher::supergraph_config::SupergraphFileEvent;
use crate::federation::Composer;
use crate::options::{LicenseAccepter, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverResult};
use apollo_federation_types::config::{FederationVersion, SchemaSource, SupergraphConfig};
use apollo_federation_types::rover::{BuildErrors, BuildOutput};
use camino::Utf8PathBuf;
use reqwest::Client;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
pub(crate) use subgraph::SubgraphSchemaWatcherKind;
use tokio::sync::mpsc::{
    channel, unbounded_channel, Receiver, Sender, UnboundedReceiver, UnboundedSender,
};
use tokio::task::AbortHandle;

mod introspect;
mod subgraph;
mod supergraph_config;

/// Watch a supergraph for changes and automatically recompose when they happen.
///
/// Also composes immediately when started.
///
/// Used by `rover dev` and `rover lsp`
#[derive(Debug)]
pub(crate) struct Watcher {
    pub(crate) composer: Composer,
    supergraph_config_file: Option<SupergraphConfigFile>,
    subgraph_sender: Sender<subgraph::Updated>,
    subgraph_updates: Receiver<subgraph::Updated>,
    polling_interval: u64,
    subgraph_watchers: HashMap<String, subgraph::Watcher>,
    client_config: StudioClientConfig,
    profile: ProfileOpt,
}

#[derive(Debug)]
struct SupergraphConfigFile {
    supergraph_config: SupergraphConfig,
    path: Utf8PathBuf,
}

impl Watcher {
    pub(crate) async fn new(
        // TODO: just take in plugin opts?
        supergraph_config: HybridSupergraphConfig,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool,
        profile: ProfileOpt,
        polling_interval: u64,
    ) -> RoverResult<Self> {
        // TODO: instead of failing instantly, report an error like any other (once we report others...)
        let resolved_supergraph_config = resolve_supergraph_config(
            supergraph_config.merged_config.clone(),
            client_config.clone(),
            &profile,
        )
        .await?;
        let supergraph_config_file =
            supergraph_config
                .file
                .map(|(supergraph_config, path)| SupergraphConfigFile {
                    supergraph_config,
                    path,
                });
        let composer = Composer::new(
            resolved_supergraph_config,
            override_install_path,
            client_config.clone(),
            elv2_license_accepter,
            skip_update,
        )
        .await?;
        // TODO: if all senders drop, we don't want them to stop forever, so we need to use more sophisticated channels
        let (subgraph_sender, subgraph_updates) = channel(1);
        let subgraph_watchers = subgraph::get_watchers(
            &client_config,
            supergraph_config.merged_config,
            subgraph_sender.clone(),
            polling_interval,
        )
        .await?;
        Ok(Self {
            supergraph_config_file,
            composer,
            subgraph_watchers,
            subgraph_sender,
            subgraph_updates,
            polling_interval,
            profile,
            client_config,
        })
    }

    pub(crate) async fn watch(mut self) -> UnboundedReceiver<Event> {
        let (send_event, events) = unbounded_channel();

        let (send_watcher_event, watcher_events) = channel(5);
        if let Some(config_file) = self.supergraph_config_file {
            let mut supergraph_updates = supergraph_config::start_watching(config_file.path).await;
            let send_watcher_event = send_watcher_event.clone();
            tokio::spawn(async move {
                let mut previous_config = config_file.supergraph_config;
                while let Some(event) = supergraph_updates.recv().await {
                    let new_config = if let SupergraphFileEvent::SupergraphChanged(config) = &event
                    {
                        Some(config.clone())
                    } else {
                        None
                    };
                    send_watcher_event
                        .send(WatcherEvent::SupergraphConfig {
                            event,
                            previous_config: previous_config.clone(),
                        })
                        .await
                        .unwrap();
                    if let Some(new_config) = new_config {
                        previous_config = new_config;
                    }
                }
            });
        }

        let watchers: HashMap<String, AbortHandle> = self
            .subgraph_watchers
            .into_iter()
            .map(|(subgraph_name, subgraph_watcher)| {
                send_event
                    .send(Event::StartedWatchingSubgraph(
                        subgraph_watcher.schema_watcher_kind.clone(),
                    ))
                    .ok();
                (
                    subgraph_name,
                    tokio::spawn(subgraph_watcher.watch_subgraph_for_changes()).abort_handle(),
                )
            })
            .collect();

        tokio::spawn(async move {
            while let Some(subgraph_event) = self.subgraph_updates.recv().await {
                send_watcher_event
                    .send(WatcherEvent::Subgraph(subgraph_event))
                    .await
                    .unwrap();
            }
        });

        tokio::spawn(
            SubWatcher {
                watchers,
                composer: self.composer,
                subgraph_sender: self.subgraph_sender,
                sender: send_event,
                receiver: watcher_events,
                client_config: self.client_config,
                profile_opt: self.profile,
                polling_interval: self.polling_interval,
            }
            .handle(),
        );
        events
    }
}

/// Watches the watchers— collects the events sent by [`subgraph::Watcher`] and \
/// [`supergraph_config::Watcher`], processes them, and emits the results as [`Event`].
struct SubWatcher {
    watchers: HashMap<String, AbortHandle>,
    composer: Composer,
    subgraph_sender: Sender<subgraph::Updated>,
    sender: UnboundedSender<Event>,
    receiver: Receiver<WatcherEvent>,
    client_config: StudioClientConfig,
    profile_opt: ProfileOpt,
    polling_interval: u64,
}

impl SubWatcher {
    /// Loops until the sender or receiver closes, then returns None.
    async fn handle(mut self) -> Option<()> {
        let client = self
            .client_config
            .get_builder()
            .with_timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        self.sender.send(self.compose(None).await).ok()?;
        while let Some(watcher_event) = self.receiver.recv().await {
            let to_send = match watcher_event {
                WatcherEvent::Subgraph(subgraph_update) => {
                    self.sender
                        .send(Event::SubgraphUpdated {
                            subgraph_name: subgraph_update.subgraph_name.clone(),
                        })
                        .ok()?;
                    self.composer.update_subgraph_sdl(
                        &subgraph_update.subgraph_name,
                        subgraph_update.new_sdl,
                    );
                    self.compose(Some(subgraph_update.subgraph_name)).await
                }
                WatcherEvent::SupergraphConfig {
                    previous_config,
                    event: SupergraphFileEvent::SupergraphChanged(new_config),
                } => {
                    let new_federation_version = new_config.get_federation_version();
                    if new_federation_version != previous_config.get_federation_version() {
                        if let Some(new_federation_version) = new_federation_version {
                            match self
                                .composer
                                .clone()
                                .set_federation_version(new_federation_version.clone())
                                .await
                            {
                                Ok(new_composer) => {
                                    self.composer = new_composer;
                                }
                                Err(err) => {
                                    self.sender
                                        .send(Event::CompositionFailed {
                                            err,
                                            federation_version: new_federation_version,
                                        })
                                        .ok()?;
                                    continue;
                                }
                            }
                        }
                    }
                    if let Err(err) = self
                        .update_subgraphs(&client, previous_config, new_config)
                        .await
                    {
                        self.sender
                            .send(Event::CompositionFailed {
                                err,
                                federation_version: self.composer.get_federation_version(),
                            })
                            .ok()?;
                    }
                    self.compose(None).await
                }
                WatcherEvent::SupergraphConfig {
                    previous_config: _previous_config,
                    event: SupergraphFileEvent::FailedToReadSupergraph(err),
                } => Event::CompositionFailed {
                    err: err.into(),
                    federation_version: self.composer.get_federation_version(),
                },
                WatcherEvent::SupergraphConfig {
                    previous_config: _previous_config,
                    event: SupergraphFileEvent::SupergraphWasInvalid(err),
                } => Event::CompositionFailed {
                    err: err.into(),
                    federation_version: self.composer.get_federation_version(),
                },
            };
            self.sender.send(to_send).ok()?
        }
        None
    }

    async fn update_subgraphs(
        &mut self,
        client: &Client,
        previous_config: SupergraphConfig,
        new_config: SupergraphConfig,
    ) -> Result<(), RoverError> {
        // TODO: decide what to do with these errors.
        let mut old_subgraphs: BTreeMap<_, _> = previous_config.into_iter().collect();
        for (subgraph_name, new_subgraph) in new_config {
            if old_subgraphs
                .remove(&subgraph_name)
                .is_some_and(|old_subgraph| old_subgraph == new_subgraph)
            {
                // Nothing changed on this one
                continue;
            }
            let schema_source = new_subgraph.schema.clone();

            // TODO: This is heavy handed, we don't need to _always_ restart, only if schema source changed
            if let Some(abort) = self.watchers.remove(&subgraph_name) {
                abort.abort()
            }
            let Some(watcher) = subgraph::Watcher::new(
                subgraph_name.clone(),
                schema_source.clone(),
                self.subgraph_sender.clone(),
                client.clone(),
                &self.client_config,
                self.polling_interval,
            ) else {
                continue;
            };
            self.sender.send(Event::StartedWatchingSubgraph(
                watcher.schema_watcher_kind.clone(),
            ))?;
            let abort_handle = tokio::spawn(watcher.watch_subgraph_for_changes()).abort_handle();
            self.watchers.insert(subgraph_name.clone(), abort_handle);

            let resolved =
                resolve_subgraph(new_subgraph, self.client_config.clone(), &self.profile_opt)
                    .await?;
            let old = self
                .composer
                .insert_subgraph(subgraph_name.clone(), resolved);
            if old.is_none() {
                self.sender.send(Event::SubgraphAdded {
                    subgraph_name,
                    schema_source,
                })?;
            }
        }
        for (subgraph_name, _old_subgraph) in old_subgraphs {
            self.composer.remove_subgraph(&subgraph_name);
            self.sender.send(Event::SubgraphRemoved { subgraph_name })?;
        }
        Ok(())
    }

    async fn compose(&self, subgraph_name: Option<String>) -> Event {
        let federation_version = self.composer.get_federation_version();
        match self.composer.compose(None).await {
            Err(err) => Event::CompositionFailed {
                err,
                federation_version,
            },
            Ok(Err(errors)) => Event::CompositionErrors {
                errors,
                federation_version,
            },
            Ok(Ok(build_output)) => Event::CompositionSucceeded {
                output: build_output,
                federation_version,
                subgraph_name,
            },
        }
    }
}

/// An event from one of our types of watchers
#[derive(Debug)]
enum WatcherEvent {
    Subgraph(subgraph::Updated),
    SupergraphConfig {
        event: SupergraphFileEvent,
        previous_config: SupergraphConfig,
    },
}

#[derive(Debug)]
pub(crate) enum Event {
    StartedWatchingSubgraph(SubgraphSchemaWatcherKind),
    /// A subgraph schema change was detected, recomposition will happen soon
    SubgraphUpdated {
        subgraph_name: String,
    },
    /// Composition could not run at all
    CompositionFailed {
        err: RoverError,
        federation_version: FederationVersion,
    },
    CompositionSucceeded {
        output: BuildOutput,
        federation_version: FederationVersion,
        /// If a particular subgraph caused this re-composition
        subgraph_name: Option<String>,
    },
    /// Composition ran, but there were errors in the subgraphs
    CompositionErrors {
        errors: BuildErrors,
        federation_version: FederationVersion,
    },
    SubgraphAdded {
        subgraph_name: String,
        schema_source: SchemaSource,
    },
    SubgraphRemoved {
        subgraph_name: String,
    },
}
