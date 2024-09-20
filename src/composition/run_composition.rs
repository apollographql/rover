use buildstructor::Builder;
use tap::TapFallible;
use tokio_stream::StreamExt;

use crate::utils::effect::{exec::ExecCommand, read_file::ReadFile};

use super::{
    events::CompositionEvent,
    supergraph::{binary::SupergraphBinary, config::FinalSupergraphConfig},
    watchers::{subtask::SubtaskHandleStream, watcher::subgraph::SubgraphChanged},
};

#[derive(Builder)]
pub struct RunComposition<ReadF, ExecC> {
    supergraph_config: FinalSupergraphConfig,
    supergraph_binary: SupergraphBinary,
    exec_command: ExecC,
    read_file: ReadF,
}

impl<ReadF, ExecC> SubtaskHandleStream for RunComposition<ReadF, ExecC>
where
    ReadF: ReadFile + Clone + Send + Sync + 'static,
    ExecC: ExecCommand + Clone + Send + Sync + 'static,
{
    type Input = SubgraphChanged;
    type Output = CompositionEvent;
    fn handle(
        self,
        sender: tokio::sync::mpsc::UnboundedSender<Self::Output>,
        mut input: futures::stream::BoxStream<'static, Self::Input>,
    ) -> tokio::task::AbortHandle {
        let supergraph_config = self.supergraph_config.clone();
        tokio::task::spawn(async move {
            while (input.next().await).is_some() {
                // this block makes sure that the read lock is dropped asap
                let output = {
                    let path = supergraph_config.read_lock().await;
                    let _ = sender
                        .send(CompositionEvent::Started)
                        .tap_err(|err| tracing::error!("{:?}", err));
                    self.supergraph_binary
                        .compose(&self.exec_command, &self.read_file, &path)
                        .await
                };
                match output {
                    Ok(success) => {
                        let _ = sender
                            .send(CompositionEvent::Success(success))
                            .tap_err(|err| tracing::error!("{:?}", err));
                    }
                    Err(err) => {
                        let _ = sender
                            .send(CompositionEvent::Error(err))
                            .tap_err(|err| tracing::error!("{:?}", err));
                    }
                }
            }
        })
        .abort_handle()
    }
}
