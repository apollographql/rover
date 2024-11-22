use apollo_federation_types::config::SupergraphConfig;
use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::stream::BoxStream;
use rover_std::errln;
use tap::TapFallible;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};
use tokio_stream::StreamExt;

use crate::{
    composition::{
        events::CompositionEvent,
        supergraph::{
            binary::{OutputTarget, SupergraphBinary},
            config::resolve::FullyResolvedSubgraphs,
        },
        watchers::subgraphs::SubgraphEvent,
    },
    subtask::SubtaskHandleStream,
    utils::effect::{exec::ExecCommand, read_file::ReadFile, write_file::WriteFile},
};

#[derive(Builder, Debug)]
pub struct CompositionWatcher<ReadF, ExecC, WriteF> {
    subgraphs: FullyResolvedSubgraphs,
    supergraph_binary: SupergraphBinary,
    output_target: OutputTarget,
    exec_command: ExecC,
    read_file: ReadF,
    write_file: WriteF,
    temp_dir: Utf8PathBuf,
}

impl<ReadF, ExecC, WriteF> SubtaskHandleStream for CompositionWatcher<ReadF, ExecC, WriteF>
where
    ReadF: ReadFile + Send + Sync + 'static,
    ExecC: ExecCommand + Send + Sync + 'static,
    WriteF: WriteFile + Send + Sync + 'static,
{
    type Input = SubgraphEvent;
    type Output = CompositionEvent;

    fn handle(
        self,
        sender: UnboundedSender<Self::Output>,
        mut input: BoxStream<'static, Self::Input>,
    ) -> AbortHandle {
        tokio::task::spawn({
            let mut subgraphs = self.subgraphs.clone();
            let target_file = self.temp_dir.join("supergraph.yaml");
            async move {
                while let Some(event) = input.next().await {
                    match event {
                        SubgraphEvent::SubgraphChanged(subgraph_schema_changed) => {
                            let name = subgraph_schema_changed.name();
                            let sdl = subgraph_schema_changed.sdl();
                            subgraphs.upsert_subgraph(name.to_string(), sdl.to_string());
                        }
                        SubgraphEvent::SubgraphRemoved(subgraph_removed) => {
                            let name = subgraph_removed.name();
                            subgraphs.remove_subgraph(name);
                        }
                    }

                    let supergraph_config = SupergraphConfig::from(subgraphs.clone());
                    let supergraph_config_yaml = serde_yaml::to_string(&supergraph_config);

                    let supergraph_config_yaml = match supergraph_config_yaml {
                        Ok(supergraph_config_yaml) => supergraph_config_yaml,
                        Err(err) => {
                            errln!("Failed to serialize supergraph config into yaml");
                            tracing::error!("{:?}", err);
                            continue;
                        }
                    };

                    let write_file_result = self
                        .write_file
                        .write_file(&target_file, supergraph_config_yaml.as_bytes())
                        .await;

                    if let Err(err) = write_file_result {
                        errln!("Failed to write the supergraph config to disk");
                        tracing::error!("{:?}", err);
                        continue;
                    }

                    let _ = sender
                        .send(CompositionEvent::Started)
                        .tap_err(|err| tracing::error!("{:?}", err));

                    let output = self
                        .supergraph_binary
                        .compose(
                            &self.exec_command,
                            &self.read_file,
                            &self.output_target,
                            target_file.clone(),
                        )
                        .await;

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
    use apollo_federation_types::config::FederationVersion;
    use camino::Utf8PathBuf;
    use futures::{
        stream::{once, BoxStream},
        StreamExt,
    };
    use mockall::predicate;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;
    use tracing_test::traced_test;

    use crate::{
        composition::{
            events::CompositionEvent,
            supergraph::{
                binary::{OutputTarget, SupergraphBinary},
                config::resolve::FullyResolvedSubgraphs,
                version::SupergraphVersion,
            },
            test::{default_composition_json, default_composition_success},
            watchers::subgraphs::{SubgraphEvent, SubgraphSchemaChanged},
        },
        subtask::{Subtask, SubtaskRunStream},
        utils::effect::{
            exec::MockExecCommand, read_file::MockReadFile, write_file::MockWriteFile,
        },
    };

    use super::CompositionWatcher;

    #[rstest]
    #[case::success(false, serde_json::to_string(&default_composition_json()).unwrap())]
    #[case::error(true, "invalid".to_string())]
    #[traced_test]
    #[tokio::test]
    async fn test_runcomposition_handle(
        #[case] composition_error: bool,
        #[case] composition_output: String,
    ) -> Result<()> {
        let temp_dir = assert_fs::TempDir::new()?;
        let temp_dir_path = Utf8PathBuf::from_path_buf(temp_dir.to_path_buf()).unwrap();

        let subgraphs = FullyResolvedSubgraphs::new(BTreeMap::new());
        let supergraph_version = SupergraphVersion::new(Version::from_str("2.8.0").unwrap());

        let supergraph_binary = SupergraphBinary::builder()
            .version(supergraph_version)
            .exe(Utf8PathBuf::from_str("some/binary").unwrap())
            .build();

        let subgraph_name = "subgraph-name".to_string();
        let subgraph_sdl = "type Query { test: String! }".to_string();

        let mut mock_exec = MockExecCommand::new();
        mock_exec
            .expect_exec_command()
            .times(1)
            .returning(move |_, _| {
                Ok(Output {
                    status: ExitStatus::default(),
                    stdout: composition_output.as_bytes().into(),
                    stderr: Vec::default(),
                })
            });

        let mut mock_read_file = MockReadFile::new();
        mock_read_file.expect_read_file().times(0);

        let expected_supergraph_sdl = format!(
            indoc::indoc! {
                r#"subgraphs:
                     {}:
                       routing_url: null
                       schema:
                         sdl: '{}'
                   federation_version: null
"#
            },
            subgraph_name, subgraph_sdl
        );
        let expected_supergraph_sdl_bytes = expected_supergraph_sdl.into_bytes();

        let mut mock_write_file = MockWriteFile::new();
        mock_write_file
            .expect_write_file()
            .times(1)
            .with(
                predicate::eq(temp_dir_path.join("supergraph.yaml")),
                predicate::eq(expected_supergraph_sdl_bytes),
            )
            .returning(|_, _| Ok(()));

        let composition_handler = CompositionWatcher::builder()
            .subgraphs(subgraphs)
            .supergraph_binary(supergraph_binary)
            .exec_command(mock_exec)
            .read_file(mock_read_file)
            .write_file(mock_write_file)
            .temp_dir(temp_dir_path)
            .output_target(OutputTarget::Stdout)
            .build();

        let subgraph_change_events: BoxStream<SubgraphEvent> = once(async {
            SubgraphEvent::SubgraphChanged(SubgraphSchemaChanged::new(subgraph_name, subgraph_sdl))
        })
        .boxed();
        let (mut composition_messages, composition_subtask) = Subtask::new(composition_handler);
        let abort_handle = composition_subtask.run(subgraph_change_events);

        // Assert we always get a composition started event.
        let next_message = composition_messages.next().await;
        assert_that!(next_message)
            .is_some()
            .is_equal_to(CompositionEvent::Started);

        // Assert we get the expected final composition event.
        if !composition_error {
            let next_message = composition_messages.next().await;
            assert_that!(next_message)
                .is_some()
                .is_equal_to(CompositionEvent::Success(default_composition_success(
                    FederationVersion::ExactFedTwo(Version::from_str("2.8.0")?),
                )));
        } else {
            assert!(matches!(
                composition_messages.next().await.unwrap(),
                CompositionEvent::Error(..)
            ));
        }

        abort_handle.abort();
        Ok(())
    }
}
