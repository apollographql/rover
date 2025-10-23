use std::fmt::Debug;

use crate::composition::{
    supergraph::config::lazy::LazilyResolvedSupergraphConfig,
    watchers::{
        composition::CompositionWatcher, subgraphs::SubgraphWatchers,
        watcher::supergraph_config::SupergraphConfigWatcher,
    },
};

pub(crate) struct SetupSubgraphWatchers;

pub(crate) struct SetupSupergraphConfigWatcher {
    pub(crate) subgraph_watchers: SubgraphWatchers,
}

pub(crate) struct SetupCompositionWatcher {
    pub(crate) supergraph_config_watcher: Option<SupergraphConfigWatcher>,
    pub(crate) subgraph_watchers: SubgraphWatchers,
    pub(crate) initial_supergraph_config: LazilyResolvedSupergraphConfig,
}

pub(crate) struct Run<ExecC, WriteF>
where
    ExecC: Eq + PartialEq + Debug,
    WriteF: Eq + PartialEq + Debug,
{
    pub(crate) supergraph_config_watcher: Option<SupergraphConfigWatcher>,
    pub(crate) subgraph_watchers: SubgraphWatchers,
    pub(crate) composition_watcher: CompositionWatcher<ExecC, WriteF>,
    pub(crate) initial_supergraph_config: LazilyResolvedSupergraphConfig,
}
