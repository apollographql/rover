use buildstructor::Builder;
use futures::stream::BoxStream;
use tap::TapFallible;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};
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
    ReadF: ReadFile + Send + Sync + 'static,
    ExecC: ExecCommand + Send + Sync + 'static,
{
    type Input = SubgraphChanged;
    type Output = CompositionEvent;

    fn handle(
        self,
        sender: UnboundedSender<Self::Output>,
        mut input: BoxStream<'static, Self::Input>,
    ) -> AbortHandle {
        let supergraph_config = self.supergraph_config.clone();
        tokio::task::spawn(async move {
            while (input.next().await).is_some() {
                // NOTE: holding the read lock makes this blocking, we should
                // ensure it is dropped asap.
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

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        process::{ExitStatus, Output},
        str::FromStr,
    };

    use anyhow::Result;
    use apollo_federation_types::config::SupergraphConfig;
    use camino::Utf8PathBuf;
    use futures::{
        stream::{once, BoxStream},
        StreamExt,
    };
    use rstest::{fixture, rstest};
    use semver::Version;

    use crate::{
        composition::{
            events::CompositionEvent,
            supergraph::{
                binary::{OutputTarget, SupergraphBinary},
                config::FinalSupergraphConfig,
                version::SupergraphVersion,
            },
            watchers::{
                subtask::{Subtask, SubtaskRunStream},
                watcher::subgraph::SubgraphChanged,
            },
        },
        utils::effect::{exec::MockExecCommand, read_file::MockReadFile},
    };

    use super::RunComposition;

    #[fixture]
    fn command_output(err: bool) -> Output {
        let status = if err {
            ExitStatus::from(1)
        } else {
            ExitStatus::default()
        };

        Output {
            status,
            stdout: "output".as_bytes().into(),
            stderr: Vec::default(),
        }
    }

    #[rstest]
    #[case::success(CompositionEvent::Success, command_output(false))]
    #[case::error(CompositionEvent::Error, command_output(true))]
    #[tokio::test]
    async fn test_runcomposition_handle(
        #[case] command_output: Output,
        #[case] composition_event: CompositionEvent,
    ) -> Result<()> {
        let supergraph_config = FinalSupergraphConfig::new(
            Some(Utf8PathBuf::from_str("/tmp/supergraph_config.yaml")?),
            Utf8PathBuf::from_str("/tmp/target/supergraph_config.yaml")?,
            SupergraphConfig::new(BTreeMap::new(), None),
        );

        let supergraph_binary = SupergraphBinary::new(
            Utf8PathBuf::from_str("/tmp/supergraph")?,
            SupergraphVersion::new(Version::from_str("2.8.0")?),
            OutputTarget::Stdout,
        );

        let mut mock_exec = MockExecCommand::new();
        mock_exec
            .expect_exec_command()
            .times(1)
            .returning(move |_, _| Ok(command_output.clone()));

        let mut mock_read_file = MockReadFile::new();
        mock_read_file.expect_read_file().times(0);

        let composition_handler = RunComposition::builder()
            .supergraph_config(supergraph_config)
            .supergraph_binary(supergraph_binary)
            .exec_command(mock_exec)
            .read_file(mock_read_file)
            .build();

        let subgraph_change_events: BoxStream<SubgraphChanged> =
            once(async { SubgraphChanged }).boxed();
        let (mut composition_messages, composition_subtask) = Subtask::new(composition_handler);
        let abort_handle = composition_subtask.run(subgraph_change_events);

        // Assert we always get a composition started event.
        assert_eq!(
            CompositionEvent::Started,
            composition_messages.next().await.unwrap()
        );

        // Assert we get the expected final composition event.
        assert_eq!(
            composition_event,
            composition_messages.next().await.unwrap()
        );

        abort_handle.abort();
        Ok(())
    }
}
