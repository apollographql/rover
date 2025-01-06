//! A [`Runner`] provides methods for configuring and handling background tasks for producing
//! composition events based of supergraph config changes.

#![warn(missing_docs)]

use std::{
    collections::{BTreeMap, HashMap},
    fmt::Debug,
};

use camino::Utf8PathBuf;
use futures::stream::{BoxStream, StreamExt};
use rover_http::HttpService;
use tower::ServiceExt;

use crate::{
    composition::watchers::watcher::{
        file::FileWatcher, supergraph_config::SupergraphConfigWatcher,
    },
    subtask::{Subtask, SubtaskRunStream, SubtaskRunUnit},
    utils::effect::{exec::ExecCommand, read_file::ReadFile, write_file::WriteFile},
};

use self::state::SetupSubgraphWatchers;

use super::{
    events::CompositionEvent,
    supergraph::{
        binary::SupergraphBinary,
        config::{
            error::ResolveSubgraphError,
            full::{introspect::MakeResolveIntrospectSubgraph, FullyResolvedSupergraphConfig},
            lazy::{LazilyResolvedSubgraph, LazilyResolvedSupergraphConfig},
            resolver::fetch_remote_subgraph::FetchRemoteSubgraphFactory,
        },
    },
    watchers::{composition::CompositionWatcher, subgraphs::SubgraphWatchers},
};

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
    state: State,
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
            let f = FileWatcher::basic(origin_path.clone());
            let watcher = SupergraphConfigWatcher::new(f, supergraph_config);
            Some(watcher)
        } else {
            None
        };
        Runner {
            state: state::SetupCompositionWatcher {
                supergraph_config_watcher,
                subgraph_watchers: self.state.subgraph_watchers,
            },
        }
    }
}

impl Runner<state::SetupCompositionWatcher> {
    /// Configures the composition watcher
    #[allow(clippy::too_many_arguments)]
    pub fn setup_composition_watcher<ExecC, ReadF, WriteF>(
        self,
        supergraph_config: FullyResolvedSupergraphConfig,
        supergraph_binary: SupergraphBinary,
        exec_command: ExecC,
        read_file: ReadF,
        write_file: WriteF,
        temp_dir: Utf8PathBuf,
    ) -> Runner<state::Run<ExecC, ReadF, WriteF>>
    where
        ExecC: ExecCommand + Debug + Eq + PartialEq + Send + Sync + 'static,
        ReadF: ReadFile + Debug + Eq + PartialEq + Send + Sync + 'static,
        WriteF: WriteFile + Debug + Eq + PartialEq + Send + Sync + 'static,
    {
        // Create a handler for supergraph composition events.
        let composition_watcher = CompositionWatcher::builder()
            .supergraph_config(supergraph_config)
            .supergraph_binary(supergraph_binary)
            .exec_command(exec_command)
            .read_file(read_file)
            .write_file(write_file)
            .temp_dir(temp_dir)
            .build();
        Runner {
            state: state::Run {
                subgraph_watchers: self.state.subgraph_watchers,
                supergraph_config_watcher: self.state.supergraph_config_watcher,
                composition_watcher,
            },
        }
    }
}

/// Alias for a [`Runner`] that is ready to be run
pub type CompositionRunner<ExecC, ReadF, WriteF> = Runner<state::Run<ExecC, ReadF, WriteF>>;

impl<ExecC, ReadF, WriteF> Runner<state::Run<ExecC, ReadF, WriteF>>
where
    ExecC: ExecCommand + Debug + Eq + PartialEq + Send + Sync + 'static,
    ReadF: ReadFile + Debug + Eq + PartialEq + Send + Sync + 'static,
    WriteF: WriteFile + Debug + Eq + PartialEq + Send + Sync + 'static,
{
    /// Runs the [`Runner`]
    pub fn run(self) -> BoxStream<'static, CompositionEvent> {
        let (supergraph_config_stream, supergraph_config_subtask) = if let Some(
            supergraph_config_watcher,
        ) =
            self.state.supergraph_config_watcher
        {
            tracing::info!("Watching subgraphs for changes...");
            let (supergraph_config_stream, supergraph_config_subtask) =
                Subtask::new(supergraph_config_watcher);
            (
                supergraph_config_stream.boxed(),
                Some(supergraph_config_subtask),
            )
        } else {
            tracing::warn!(
                    "No supergraph config detected, changes to subgraph configurations will not be applied automatically"
                );
            (tokio_stream::empty().boxed(), None)
        };

        let (subgraph_change_stream, subgraph_watcher_subtask) =
            Subtask::new(self.state.subgraph_watchers);

        // Create a new subtask for the composition handler, passing in a stream of subgraph change
        // events in order to trigger recomposition.
        let (composition_messages, composition_subtask) =
            Subtask::new(self.state.composition_watcher);
        composition_subtask.run(subgraph_change_stream.boxed());

        // Start subgraph watchers, listening for events from the supergraph change stream.
        subgraph_watcher_subtask.run(supergraph_config_stream);

        // Start the supergraph watcher subtask.
        if let Some(supergraph_config_subtask) = supergraph_config_subtask {
            supergraph_config_subtask.run();
        }

        composition_messages.boxed()
    }
}
