use crate::federation::supergraph_config::{resolve_supergraph_config, HybridSupergraphConfig};
use crate::federation::watcher::supergraph_config::SupergraphFileEvent;
use crate::federation::Composer;
use crate::options::{LicenseAccepter, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverResult};
use apollo_federation_types::config::SupergraphConfig;
use apollo_federation_types::rover::{BuildErrors, BuildOutput};
use camino::Utf8PathBuf;
use std::collections::HashMap;
pub(crate) use subgraph::SubgraphSchemaWatcherKind;
use tokio::sync::mpsc::{channel, unbounded_channel, Receiver, UnboundedReceiver};

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
    subgraph_updates: Receiver<subgraph::Updated>,
    subgraph_watchers: HashMap<String, subgraph::Watcher>,
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
        profile: &ProfileOpt,
        polling_interval: u64,
    ) -> RoverResult<Self> {
        // TODO: instead of failing instantly, report an error like any other (once we report others...)
        let resolved_supergraph_config = resolve_supergraph_config(
            supergraph_config.merged_config.clone(),
            client_config.clone(),
            profile,
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
            subgraph_updates,
        })
    }

    pub(crate) async fn watch(mut self) -> UnboundedReceiver<Event> {
        let (send_event, events) = unbounded_channel();

        let (send_watcher_event, mut watcher_events) = channel(5);
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

        // TODO: find a way to stop old watchers if subgraphs are removed
        for (_, subgraph_watcher) in self.subgraph_watchers.into_iter() {
            send_event
                .send(Event::StartedWatchingSubgraph(
                    subgraph_watcher.schema_watcher_kind.clone(),
                ))
                .ok();
            tokio::spawn(subgraph_watcher.watch_subgraph_for_changes());
        }

        tokio::spawn(async move {
            while let Some(subgraph_event) = self.subgraph_updates.recv().await {
                send_watcher_event
                    .send(WatcherEvent::Subgraph(subgraph_event))
                    .await
                    .unwrap();
            }
        });

        tokio::spawn(async move {
            send_event
                .send(compose(&self.composer, None).await)
                .unwrap(); // TODO: don't panic, just return, it's cool
            while let Some(watcher_event) = watcher_events.recv().await {
                match watcher_event {
                    WatcherEvent::Subgraph(subgraph_update) => {
                        send_event
                            .send(Event::SubgraphUpdated {
                                subgraph_name: subgraph_update.subgraph_name.clone(),
                            })
                            .unwrap();
                        self.composer.update_subgraph_sdl(
                            &subgraph_update.subgraph_name,
                            subgraph_update.new_sdl,
                        );
                        send_event
                            .send(
                                compose(&self.composer, Some(subgraph_update.subgraph_name)).await,
                            )
                            .unwrap(); // TODO: send error is actually ok, just exit
                    }
                    WatcherEvent::SupergraphConfig {
                        previous_config,
                        event: SupergraphFileEvent::SupergraphChanged(supergraph_config),
                    } => {
                        let new_federation_version = supergraph_config.get_federation_version();
                        if new_federation_version != previous_config.get_federation_version() {
                            if let Some(new_federation_version) = new_federation_version {
                                // TODO: If there's an error, report it somewhere
                                if let Ok(new_composer) = self
                                    .composer
                                    .clone()
                                    .set_federation_version(new_federation_version)
                                    .await
                                {
                                    self.composer = new_composer;
                                }
                            }
                            send_event
                                // TODO: this isn't initial composition, but do we care?
                                .send(compose(&self.composer, None).await)
                                .unwrap(); // TODO: send error is actually ok, just exit
                        }
                    }
                    WatcherEvent::SupergraphConfig {
                        previous_config: _previous_config,
                        event: SupergraphFileEvent::FailedToReadSupergraph(_err),
                    } => {
                        // TODO: handle some notification about this failure?
                        continue;
                    }
                    WatcherEvent::SupergraphConfig {
                        previous_config: _previous_config,
                        event: SupergraphFileEvent::SupergraphWasInvalid(_err),
                    } => {
                        // TODO: handle some notification about this failure?
                        continue;
                    }
                }
            }
        });
        events
    }
}

async fn compose(composer: &Composer, subgraph_name: Option<String>) -> Event {
    match composer.compose(None).await {
        Err(rover_error) => Event::CompositionFailed(rover_error),
        Ok(Err(build_errors)) => Event::CompositionErrors(build_errors),
        Ok(Ok(build_output)) => {
            if let Some(subgraph_name) = subgraph_name {
                Event::ComposedAfterSubgraphUpdated {
                    subgraph_name,
                    output: build_output,
                }
            } else {
                Event::InitialComposition(build_output)
            }
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
    CompositionFailed(RoverError),
    /// The first composition succeeded, not due to any particular update
    InitialComposition(BuildOutput),
    /// Composition ran successfully
    ComposedAfterSubgraphUpdated {
        subgraph_name: String,
        output: BuildOutput,
    },
    /// Composition ran, but there were errors in the subgraphs
    CompositionErrors(BuildErrors),
}
