use std::fmt::Debug;

use crate::composition::supergraph::config::lazy::LazilyResolvedSupergraphConfig;
use crate::composition::watchers::{
    composition::CompositionWatcher, subgraphs::SubgraphWatchers,
    watcher::supergraph_config::SupergraphConfigWatcher,
};

pub struct SetupSubgraphWatchers;

pub struct SetupSupergraphConfigWatcher {
    pub subgraph_watchers: SubgraphWatchers,
}

pub struct SetupCompositionWatcher {
    pub supergraph_config_watcher: Option<SupergraphConfigWatcher>,
    pub subgraph_watchers: SubgraphWatchers,
    pub initial_supergraph_config: LazilyResolvedSupergraphConfig,
}

pub struct Run<ExecC, WriteF>
where
    ExecC: Eq + PartialEq + Debug,
    WriteF: Eq + PartialEq + Debug,
{
    pub supergraph_config_watcher: Option<SupergraphConfigWatcher>,
    pub subgraph_watchers: SubgraphWatchers,
    pub composition_watcher: CompositionWatcher<ExecC, WriteF>,
    pub initial_supergraph_config: LazilyResolvedSupergraphConfig,
}
