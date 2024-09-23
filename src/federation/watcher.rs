use crate::federation::supergraph_config::resolve_supergraph_config;
use crate::federation::Composer;
use crate::options::{LicenseAccepter, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverResult};
use apollo_federation_types::config::SupergraphConfig;
use apollo_federation_types::rover::{BuildErrors, BuildOutput};
use camino::Utf8PathBuf;
use std::collections::HashMap;
use tokio::sync::mpsc::{channel, Receiver};

mod introspect;
mod subgraph;

/// Watch a supergraph for changes and automatically recompose when they happen.
///
/// Also composes immediately when started.
///
/// Used by `rover dev` and `rover lsp`
#[derive(Debug)]
pub(crate) struct Watcher {
    pub(crate) composer: Composer,
    pub(crate) supergraph_config: SupergraphConfig,
    subgraph_updates: Receiver<subgraph::Updated>,
    subgraph_watchers: HashMap<String, subgraph::Watcher>,
}

impl Watcher {
    pub(crate) async fn new(
        supergraph_config: SupergraphConfig,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool,
        profile: &ProfileOpt,
        polling_interval: u64,
    ) -> RoverResult<Self> {
        let resolved_supergraph_config =
            resolve_supergraph_config(supergraph_config.clone(), client_config.clone(), profile)
                .await?;
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
            supergraph_config.clone(),
            subgraph_sender.clone(),
            polling_interval,
        )
        .await?;
        Ok(Self {
            composer,
            supergraph_config,
            subgraph_watchers,
            subgraph_updates,
        })
    }

    pub(crate) async fn watch(mut self) -> Receiver<Event> {
        let (tx, rx) = channel(1);
        // TODO: find a way to stop old watchers if subgraphs are removed
        for (_, subgraph_watcher) in self.subgraph_watchers.into_iter() {
            tokio::spawn(subgraph_watcher.watch_subgraph_for_changes());
        }
        tokio::spawn(async move {
            tx.send(compose(&self.composer, None).await).await.unwrap();
            while let Some(subgraph_update) = self.subgraph_updates.recv().await {
                tx.send(Event::SubgraphUpdated {
                    subgraph_name: subgraph_update.subgraph_name.clone(),
                })
                .await
                .unwrap();
                let Some(subgraph) = self
                    .composer
                    .supergraph_config
                    .subgraphs
                    .get_mut(&subgraph_update.subgraph_name)
                else {
                    continue; // TODO: This is an error of some sort
                };
                subgraph.schema.sdl = subgraph_update.new_sdl;
                tx.send(compose(&self.composer, Some(subgraph_update.subgraph_name)).await)
                    .await
                    .unwrap();
            }
        });
        rx
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

pub(crate) enum Event {
    /// A subgraph schema change was detected, recomposition will happen soon
    SubgraphUpdated { subgraph_name: String },
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
