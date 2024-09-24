use apollo_federation_types::config::SupergraphConfig;
use futures::stream::{empty, StreamExt};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::{
    composition::watchers::{
        subtask::{Subtask, SubtaskRunUnit},
        watcher::{file::FileWatcher, supergraph_config::SupergraphConfigWatcher},
    },
    utils::effect::{exec::TokioCommand, read_file::FsReadFile},
    RoverResult,
};

use super::{
    events::CompositionEvent,
    run_composition::RunComposition,
    subgraph_watchers::SubgraphWatchers,
    supergraph::{binary::SupergraphBinary, config::FinalSupergraphConfig},
    watchers::{subtask::SubtaskRunStream, watcher::supergraph_config::SupergraphConfigDiff},
};

// TODO: handle retry flag for subgraphs (see rover dev help)
pub struct Runner {
    supergraph_config: FinalSupergraphConfig,
    supergraph_binary: SupergraphBinary,
}

impl Runner {
    pub fn new(
        supergraph_config: FinalSupergraphConfig,
        supergraph_binary: SupergraphBinary,
    ) -> Self {
        Self {
            supergraph_config,
            supergraph_binary,
        }
    }

    pub async fn run(self) -> RoverResult<UnboundedReceiverStream<CompositionEvent>> {
        let (supergraph_config_stream, supergraph_config_subtask) =
            match self.supergraph_config_subtask() {
                Some((supergraph_diff_stream, supergraph_config_subtask)) => (
                    supergraph_diff_stream.boxed(),
                    Some(supergraph_config_subtask),
                ),
                None => (empty().boxed(), None),
            };

        let subgraph_config_watchers = SubgraphWatchers::new(self.supergraph_config.clone().into());
        let (subgraph_changed_messages, subgraph_config_watchers_subtask) =
            Subtask::new(subgraph_config_watchers);

        let composition_handler = RunComposition::builder()
            .supergraph_config(self.supergraph_config)
            .supergraph_binary(self.supergraph_binary)
            .exec_command(TokioCommand::default())
            .read_file(FsReadFile::default())
            .build();
        let (composition_messages, composition_subtask) = Subtask::new(composition_handler);

        composition_subtask.run(subgraph_changed_messages.boxed());
        subgraph_config_watchers_subtask.run(supergraph_config_stream);
        if let Some(supergraph_config_subtask) = supergraph_config_subtask {
            supergraph_config_subtask.run();
        }

        Ok(composition_messages)
    }

    fn supergraph_config_subtask(
        &self,
    ) -> Option<(
        UnboundedReceiverStream<SupergraphConfigDiff>,
        Subtask<SupergraphConfigWatcher, SupergraphConfigDiff>,
    )> {
        let supergraph_config: SupergraphConfig = self.supergraph_config.clone().into();

        if let Some(origin_path) = self.supergraph_config.origin_path() {
            let f = FileWatcher::new(origin_path.clone());
            let watcher = SupergraphConfigWatcher::new(f, supergraph_config.clone());
            Some(Subtask::new(watcher))
        } else {
            None
        }
    }
}
