use std::collections::BTreeMap;

use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use buildstructor::Builder;
use camino::Utf8PathBuf;
use futures::stream::BoxStream;
use rover_std::{errln, infoln, warnln};
use tap::TapFallible;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

use crate::composition::supergraph::config::error::ResolveSubgraphError;
use crate::composition::supergraph::config::resolver::ResolveSupergraphConfigError;
use crate::composition::supergraph::install::InstallSupergraph;
use crate::composition::watchers::composition::CompositionInputEvent::{
    Federation, Passthrough, Recompose, Subgraph,
};
use crate::composition::CompositionError::ResolvingSubgraphsError;
use crate::composition::{
    CompositionError, CompositionSubgraphAdded, CompositionSubgraphRemoved, CompositionSuccess,
    FederationUpdaterConfig,
};
use crate::utils::effect::install::InstallBinary;
use crate::{
    composition::{
        events::CompositionEvent,
        supergraph::{
            binary::{OutputTarget, SupergraphBinary},
            config::full::FullyResolvedSupergraphConfig,
        },
        watchers::subgraphs::SubgraphEvent,
    },
    subtask::SubtaskHandleStream,
    utils::effect::{exec::ExecCommand, read_file::ReadFile, write_file::WriteFile},
};

/// Event to represent an input to the CompositionWatcher, depending on the source the event comes
/// from. This is really like a Union type over multiple disparate events
pub enum CompositionInputEvent {
    /// Variant to represent if the change comes from a change to subgraphs
    Subgraph(SubgraphEvent),
    /// Variant to represent if the change comes from a change in the Federation Version
    Federation(FederationVersion),
    /// Variant to represent if we need to recompose quickly without other changes
    Recompose(),
    /// Variant to something that we do not want to perform Composition on but needs to be passed
    /// through to the final stream of Composition Events.
    Passthrough(CompositionEvent),
}

#[derive(Builder, Debug)]
pub struct CompositionWatcher<ExecC, ReadF, WriteF> {
    initial_supergraph_config: FullyResolvedSupergraphConfig,
    initial_resolution_errors: BTreeMap<String, ResolveSubgraphError>,
    federation_updater_config: Option<FederationUpdaterConfig>,
    supergraph_binary: SupergraphBinary,
    exec_command: ExecC,
    read_file: ReadF,
    write_file: WriteF,
    temp_dir: Utf8PathBuf,
    compose_on_initialisation: bool,
    output_target: OutputTarget,
}

impl<ExecC, ReadF, WriteF> SubtaskHandleStream for CompositionWatcher<ExecC, ReadF, WriteF>
where
    ExecC: ExecCommand + Send + Sync + 'static,
    ReadF: ReadFile + Send + Sync + 'static,
    WriteF: WriteFile + Send + Sync + 'static,
{
    type Input = CompositionInputEvent;
    type Output = CompositionEvent;

    fn handle(
        mut self,
        sender: UnboundedSender<Self::Output>,
        mut input: BoxStream<'static, Self::Input>,
        cancellation_token: Option<CancellationToken>,
    ) {
        tokio::task::spawn(async move {
            let mut supergraph_config = self.initial_supergraph_config.clone();
            let target_file = self.temp_dir.join("supergraph.yaml");

            match (
                self.initial_resolution_errors.is_empty(),
                self.compose_on_initialisation,
            ) {
                (true, true) => {
                    if let Err(err) = self
                        .setup_temporary_supergraph_yaml(&supergraph_config, &target_file)
                        .await
                    {
                        error!("Could not setup initial supergraph schema: {}", err);
                    };
                    let _ = sender
                        .send(CompositionEvent::Started)
                        .tap_err(|err| error!("{:?}", err));
                    let output = self
                        .run_composition(&target_file, &self.output_target)
                        .await;
                    match output {
                        Ok(success) => {
                            let _ = sender
                                .send(CompositionEvent::Success(success))
                                .tap_err(|err| error!("{:?}", err));
                        }
                        Err(err) => {
                            let _ = sender
                                .send(CompositionEvent::Error(err))
                                .tap_err(|err| error!("{:?}", err));
                        }
                    }
                }
                (false, _) => {
                    let _ = sender
                        .send(CompositionEvent::Error(ResolvingSubgraphsError(
                            ResolveSupergraphConfigError::ResolveSubgraphs(
                                self.initial_resolution_errors.clone(),
                            ),
                        )))
                        .tap_err(|err| error!("{:?}", err));
                }
                (true, false) => {}
            };

            let cancellation_token = cancellation_token.unwrap_or_default();
            cancellation_token.run_until_cancelled(async {
                while let Some(event) = input.next().await {
                    match event {
                        Subgraph(SubgraphEvent::SubgraphSchemaChanged(subgraph_schema_changed)) => {
                            let name = subgraph_schema_changed.name().clone();
                            let schema_source = subgraph_schema_changed.schema_source().clone();
                            let message = format!("Schema change detected for subgraph: {}", &name);
                            infoln!("{}", message);
                            tracing::info!(message);
                            if supergraph_config
                                .update_subgraph_schema(
                                    name.clone(),
                                    subgraph_schema_changed.into(),
                                )
                                .is_none()
                            {
                                let _ = sender
                                    .send(CompositionEvent::SubgraphAdded(
                                        CompositionSubgraphAdded {
                                            name,
                                            schema_source
                                        },
                                    ))
                                    .tap_err(|err| error!("{:?}", err));
                            };
                        }
                        Subgraph(SubgraphEvent::RoutingUrlChanged(routing_url_changed)) => {
                            let name = routing_url_changed.name();
                            info!("Change of routing_url detected for subgraph '{}'", name);
                            if supergraph_config.update_routing_url(name, routing_url_changed.routing_url().clone()).is_none() {
                                // If we get None back then continue, as we don't need to recompose
                                continue
                            }
                        }
                        Subgraph(SubgraphEvent::SubgraphRemoved(subgraph_removed)) => {
                            let name = subgraph_removed.name();
                            let resolution_error = subgraph_removed.resolution_error().clone();
                            info!("Subgraph removed: {}", name);
                            supergraph_config.remove_subgraph(name);
                            let _ = sender
                                .send(CompositionEvent::SubgraphRemoved(
                                    CompositionSubgraphRemoved { name: name.clone(), resolution_error },
                                ))
                                .tap_err(|err| error!("{:?}", err));
                        }
                        Federation(fed_version) => {
                            if let Some(federation_updater_config) = self.federation_updater_config.clone() {
                                info!("Attempting to change supergraph version to {:?}", fed_version);
                                infoln!("Attempting to change supergraph version to {}", fed_version.get_exact().unwrap());
                                let install_res =
                                    InstallSupergraph::new(fed_version, federation_updater_config.studio_client_config.clone())
                                        .install(None, federation_updater_config.elv2_licence_accepter, federation_updater_config.skip_update)
                                        .await;
                                match install_res {
                                    Ok(supergraph_binary) => {
                                        tracing::info!("Supergraph version changed to {:?}", supergraph_binary.version());
                                        infoln!("Supergraph version changed to {}", supergraph_binary.version().to_string());
                                        self.supergraph_binary = supergraph_binary
                                    }
                                    Err(err) => {
                                        tracing::warn!("Failed to change supergraph version, current version has been retained...");
                                        warnln!("Failed to change supergraph version, current version has been retained...");
                                        let _ = sender.send(CompositionEvent::Error(err.into())).tap_err(|err| error!("{:?}", err));
                                        continue;
                                    }
                                }
                            } else {
                                tracing::warn!("Detected Federation Version change but due to overrides (CLI flags, ENV vars etc.) this was not actioned.")
                            }
                        },
                        // Empty because we just want to recompose what exists, not do anything else
                        Recompose() => {},
                        Passthrough(ev) => {
                            let _ = sender.send(ev).tap_err(|err| error!("{:?}", err));
                            continue;
                        }
                    }

                    if let Err(err) = self
                        .setup_temporary_supergraph_yaml(&supergraph_config, &target_file)
                        .await
                    {
                        error!("Could not setup supergraph schema: {}", err);
                        continue;
                    };

                    let _ = sender
                        .send(CompositionEvent::Started)
                        .tap_err(|err| error!("{:?}", err));

                    let output = self
                        .run_composition(&target_file, &self.output_target)
                        .await;

                    match output {
                        Ok(success) => {
                            infoln!("Composition successful.");
                            let _ = sender
                                .send(CompositionEvent::Success(success))
                                .tap_err(|err| error!("{:?}", err));
                        }
                        Err(err) => {
                            errln!("Composition failed.");
                            let _ = sender
                                .send(CompositionEvent::Error(err))
                                .tap_err(|err| error!("{:?}", err));
                        }
                    }
                }
            }).await;
        });
    }
}

impl<ExecC, ReadF, WriteF> CompositionWatcher<ExecC, ReadF, WriteF>
where
    ExecC: 'static + ExecCommand + Send + Sync,
    ReadF: 'static + ReadFile + Send + Sync,
    WriteF: 'static + Send + Sync + WriteFile,
{
    async fn setup_temporary_supergraph_yaml(
        &self,
        supergraph_config: &FullyResolvedSupergraphConfig,
        target_file: &Utf8PathBuf,
    ) -> Result<(), CompositionError> {
        let supergraph_config = SupergraphConfig::from(supergraph_config.clone());
        let supergraph_config_yaml = serde_yaml::to_string(&supergraph_config);

        let supergraph_config_yaml = match supergraph_config_yaml {
            Ok(supergraph_config_yaml) => supergraph_config_yaml,
            Err(err) => {
                errln!("Failed to serialize supergraph config into yaml");
                error!("{:?}", err);
                return Err(CompositionError::SerdeYaml(err));
            }
        };

        let write_file_result = self
            .write_file
            .write_file(target_file, supergraph_config_yaml.as_bytes())
            .await;

        if let Err(err) = write_file_result {
            errln!("Failed to write the supergraph config to disk");
            error!("{:?}", err);
            Err(CompositionError::WriteFile {
                path: target_file.clone(),
                error: Box::new(err),
            })
        } else {
            Ok(())
        }
    }
    async fn run_composition(
        &self,
        target_file: &Utf8PathBuf,
        output_target: &OutputTarget,
    ) -> Result<CompositionSuccess, CompositionError> {
        self.supergraph_binary
            .compose(
                &self.exec_command,
                &self.read_file,
                output_target,
                target_file.clone(),
            )
            .await
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
    use apollo_federation_types::config::{FederationVersion, SchemaSource};
    use camino::Utf8PathBuf;
    use futures::{
        stream::{once, BoxStream},
        StreamExt,
    };
    use mockall::predicate;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;
    use tokio_util::sync::CancellationToken;
    use tracing_test::traced_test;

    use super::{CompositionInputEvent, CompositionWatcher};
    use crate::composition::supergraph::binary::OutputTarget;
    use crate::composition::watchers::composition::CompositionInputEvent::Subgraph;
    use crate::composition::CompositionSubgraphAdded;
    use crate::subtask::SubtaskRunStream;
    use crate::{
        composition::{
            events::CompositionEvent,
            supergraph::{
                binary::SupergraphBinary, config::full::FullyResolvedSupergraphConfig,
                version::SupergraphVersion,
            },
            test::{default_composition_json, default_composition_success},
            watchers::subgraphs::{SubgraphEvent, SubgraphSchemaChanged},
        },
        subtask::Subtask,
        utils::effect::{
            exec::MockExecCommand, read_file::MockReadFile, write_file::MockWriteFile,
        },
    };

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

        let federation_version = Version::from_str("2.8.0").unwrap();

        let subgraphs = FullyResolvedSupergraphConfig::builder()
            .subgraphs(BTreeMap::new())
            .federation_version(FederationVersion::ExactFedTwo(federation_version.clone()))
            .build();
        let supergraph_version = SupergraphVersion::new(federation_version.clone());

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
            .returning(move |_| {
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
                       routing_url: https://example.com
                       schema:
                         sdl: '{}'
                   federation_version: ={}
"#
            },
            subgraph_name, subgraph_sdl, federation_version
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
            .initial_supergraph_config(subgraphs)
            .supergraph_binary(supergraph_binary)
            .exec_command(mock_exec)
            .read_file(mock_read_file)
            .write_file(mock_write_file)
            .temp_dir(temp_dir_path)
            .compose_on_initialisation(false)
            .output_target(OutputTarget::Stdout)
            .build();

        let subgraph_change_events: BoxStream<CompositionInputEvent> = once(async {
            Subgraph(SubgraphEvent::SubgraphSchemaChanged(
                SubgraphSchemaChanged::new(
                    subgraph_name,
                    subgraph_sdl.clone(),
                    "https://example.com".to_string(),
                    SchemaSource::Sdl { sdl: subgraph_sdl },
                ),
            ))
        })
        .boxed();
        let (mut composition_messages, composition_subtask) = Subtask::new(composition_handler);
        let cancellation_token = CancellationToken::new();
        composition_subtask.run(subgraph_change_events, Some(cancellation_token.clone()));

        // Assert we always get a subgraph added event.
        let next_message = composition_messages.next().await;
        assert_that!(next_message).is_some().matches(|event| {
            matches!(
                event,
                CompositionEvent::SubgraphAdded(CompositionSubgraphAdded { .. })
            )
        });

        // Assert we always get a composition started event.
        let next_message = composition_messages.next().await;
        assert_that!(next_message)
            .is_some()
            .matches(|event| matches!(event, CompositionEvent::Started));

        // Assert we get the expected final composition event.
        if !composition_error {
            let next_message = composition_messages.next().await;
            assert_that!(next_message).is_some().matches(|event| {
                if let CompositionEvent::Success(success) = event {
                    success
                        == &default_composition_success(FederationVersion::ExactFedTwo(
                            Version::from_str("2.8.0").unwrap(),
                        ))
                } else {
                    false
                }
            });
        } else {
            assert!(matches!(
                composition_messages.next().await.unwrap(),
                CompositionEvent::Error(..)
            ));
        }

        cancellation_token.cancel();
        Ok(())
    }
}
