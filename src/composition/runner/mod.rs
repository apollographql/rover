//! A [`Runner`] provides methods for configuring and handling background tasks for producing
//! composition events based of supergraph config changes.

#![warn(missing_docs)]

use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

use camino::Utf8PathBuf;
use futures::stream::{BoxStream, StreamExt, select};
use rover_http::HttpService;
use tower::ServiceExt;

use self::state::SetupSubgraphWatchers;
use super::FederationUpdaterConfig;
use super::events::CompositionEvent;
use super::supergraph::binary::SupergraphBinary;
use super::supergraph::config::error::ResolveSubgraphError;
use super::supergraph::config::full::FullyResolvedSupergraphConfig;
use super::supergraph::config::full::introspect::MakeResolveIntrospectSubgraph;
use super::supergraph::config::lazy::{LazilyResolvedSubgraph, LazilyResolvedSupergraphConfig};
use super::supergraph::config::resolver::fetch_remote_subgraph::FetchRemoteSubgraphFactory;
use super::watchers::composition::CompositionWatcher;
use super::watchers::subgraphs::SubgraphWatchers;
use crate::composition::supergraph::config::full::introspect::ResolveIntrospectSubgraphFactory;
use crate::composition::supergraph::install::InstallSupergraphError;
use crate::composition::watchers::federation::FederationWatcher;
use crate::composition::watchers::watcher::file::FileWatcher;
use crate::composition::watchers::watcher::supergraph_config::SupergraphConfigWatcher;
use crate::subtask::{BroadcastSubtask, Subtask, SubtaskRunStream, SubtaskRunUnit};
use crate::utils::effect::exec::ExecCommand;
use crate::utils::effect::write_file::WriteFile;

mod state;

/// A struct for configuring and running subtasks for watching for both supergraph and subgraph
/// change events.
/// This is parameterized around the values in the [`state`] module, as to provide
/// a type-based workflow for configuring and running the [`Runner`]
///
/// The configuration flow goes as follows:
/// Runner<SetupSubgraphWatchers>
///   -> Runner<SetupSupergraphConfigWatcher>
///   -> Runner<SetupCompositionWatcher>
///   -> Runner<Run>
// TODO: handle retry flag for subgraphs (see rover dev help)
pub struct Runner<State> {
    pub(crate) state: State,
}

impl Default for Runner<SetupSubgraphWatchers> {
    fn default() -> Self {
        Runner {
            state: state::SetupSubgraphWatchers,
        }
    }
}

impl Runner<state::SetupSubgraphWatchers> {
    /// Configures the subgraph watchers for the [`Runner`]
    pub async fn setup_subgraph_watchers(
        self,
        subgraphs: BTreeMap<String, LazilyResolvedSubgraph>,
        http_service: HttpService,
        fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
        supergraph_config_root: Utf8PathBuf,
        introspection_polling_interval: u64,
    ) -> Result<Runner<state::SetupSupergraphConfigWatcher>, HashMap<String, ResolveSubgraphError>>
    {
        let resolve_introspect_subgraph_factory =
            MakeResolveIntrospectSubgraph::new(http_service).boxed_clone();
        let subgraph_watchers = SubgraphWatchers::new(
            subgraphs,
            resolve_introspect_subgraph_factory,
            fetch_remote_subgraph_factory,
            &supergraph_config_root,
            introspection_polling_interval,
        )
        .await?;
        Ok(Runner {
            state: state::SetupSupergraphConfigWatcher { subgraph_watchers },
        })
    }
}

impl Runner<state::SetupSupergraphConfigWatcher> {
    /// Configures the supergraph watcher for the [`Runner`]
    pub fn setup_supergraph_config_watcher(
        self,
        supergraph_config: LazilyResolvedSupergraphConfig,
        fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory,
        resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory,
    ) -> Runner<state::SetupCompositionWatcher> {
        // If the supergraph config was passed as a file, we can configure a watcher for change
        // events.
        // We could return None here if we received a supergraph config directly from stdin. In
        // that case, we don't want to configure a watcher.
        tracing::info!(
            "Setting up SupergraphConfigWatcher from origin: {}",
            supergraph_config
                .origin_path()
                .as_ref()
                .map(|x| x.to_string())
                .unwrap_or_default()
        );
        let supergraph_config_watcher = if let Some(origin_path) = supergraph_config.origin_path() {
            let f = FileWatcher::new(origin_path.clone());
            let watcher = SupergraphConfigWatcher::new(
                f,
                supergraph_config.clone(),
                fetch_remote_subgraph_factory,
                resolve_introspect_subgraph_factory,
            );
            Some(watcher)
        } else {
            None
        };
        Runner {
            state: state::SetupCompositionWatcher {
                supergraph_config_watcher,
                subgraph_watchers: self.state.subgraph_watchers,
                initial_supergraph_config: supergraph_config,
            },
        }
    }
}

impl Runner<state::SetupCompositionWatcher> {
    /// Configures the composition watcher
    #[allow(clippy::too_many_arguments)]
    pub fn setup_composition_watcher<ExecC, WriteF>(
        self,
        initial_supergraph_config: FullyResolvedSupergraphConfig,
        initial_resolution_errors: BTreeMap<String, ResolveSubgraphError>,
        supergraph_binary: Result<SupergraphBinary, InstallSupergraphError>,
        exec_command: ExecC,
        write_file: WriteF,
        temp_dir: Utf8PathBuf,
        compose_on_initialisation: bool,
        federation_updater_config: Option<FederationUpdaterConfig>,
    ) -> Runner<state::Run<ExecC, WriteF>>
    where
        ExecC: ExecCommand + Debug + Eq + PartialEq + Send + Sync + 'static,
        WriteF: WriteFile + Debug + Eq + PartialEq + Send + Sync + 'static,
    {
        // Create a handler for supergraph composition events.
        let composition_watcher_builder = CompositionWatcher::builder()
            .initial_supergraph_config(initial_supergraph_config)
            .initial_resolution_errors(initial_resolution_errors)
            .supergraph_binary(supergraph_binary)
            .exec_command(exec_command)
            .write_file(write_file)
            .temp_dir(temp_dir)
            .compose_on_initialisation(compose_on_initialisation);

        let composition_watcher = if let Some(federation_updater_config) = federation_updater_config
        {
            composition_watcher_builder
                .federation_updater_config(federation_updater_config)
                .build()
        } else {
            composition_watcher_builder.build()
        };

        Runner {
            state: state::Run {
                subgraph_watchers: self.state.subgraph_watchers,
                supergraph_config_watcher: self.state.supergraph_config_watcher,
                composition_watcher,
                initial_supergraph_config: self.state.initial_supergraph_config,
            },
        }
    }
}

/// Alias for a [`Runner`] that is ready to be run
pub type CompositionRunner<ExecC, WriteF> = Runner<state::Run<ExecC, WriteF>>;

impl<ExecC, WriteF> Runner<state::Run<ExecC, WriteF>>
where
    ExecC: ExecCommand + Debug + Eq + PartialEq + Send + Sync + 'static,
    WriteF: WriteFile + Debug + Eq + PartialEq + Send + Sync + 'static,
{
    /// Runs the [`Runner`]
    pub fn run(self) -> BoxStream<'static, CompositionEvent> {
        let (
            supergraph_config_stream_for_subtask_watcher,
            supergraph_config_stream_for_federation_watcher,
            supergraph_config_subtask,
        ) = if let Some(supergraph_config_watcher) = self.state.supergraph_config_watcher {
            tracing::info!("Watching subgraphs for changes...");
            let (supergraph_config_stream, supergraph_config_subtask) =
                BroadcastSubtask::new(supergraph_config_watcher);
            (
                supergraph_config_stream.boxed(),
                supergraph_config_subtask.subscribe().boxed(),
                Some(supergraph_config_subtask),
            )
        } else {
            tracing::warn!(
                "No supergraph config detected, changes to subgraph configurations will not be applied automatically"
            );
            (
                tokio_stream::empty().boxed(),
                tokio_stream::empty().boxed(),
                None,
            )
        };

        let (subgraph_change_stream, subgraph_watcher_subtask) =
            Subtask::new(self.state.subgraph_watchers);

        let (federation_watcher_stream, federation_watcher_subtask) =
            Subtask::new(FederationWatcher {});

        // Create a new subtask for the composition handler, passing in a stream of subgraph change
        // events in order to trigger recomposition.
        let (composition_messages, composition_subtask) =
            Subtask::new(self.state.composition_watcher);
        composition_subtask.run(
            select(subgraph_change_stream, federation_watcher_stream).boxed(),
            None,
        );

        // Start subgraph watchers, listening for events from the supergraph change stream.
        subgraph_watcher_subtask.run(
            supergraph_config_stream_for_subtask_watcher
                .filter_map(|recv_res| async move {
                    match recv_res {
                        Ok(res) => Some(res),
                        Err(e) => {
                            tracing::warn!("Error receiving from broadcast stream: {:?}", e);
                            None
                        }
                    }
                })
                .boxed(),
            None,
        );

        federation_watcher_subtask.run(
            supergraph_config_stream_for_federation_watcher
                .filter_map(|recv_res| async move {
                    match recv_res {
                        Ok(res) => Some(res),
                        Err(e) => {
                            tracing::warn!("Error receiving from broadcast stream: {:?}", e);
                            None
                        }
                    }
                })
                .boxed(),
            None,
        );

        // Start the supergraph watcher subtask.
        if let Some(supergraph_config_subtask) = supergraph_config_subtask {
            supergraph_config_subtask.run(None);
        }

        composition_messages.boxed()
    }
}
