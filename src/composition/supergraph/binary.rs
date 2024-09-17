use std::{fmt::Debug, str::Utf8Error};

use camino::Utf8PathBuf;
use tap::TapFallible;

use crate::utils::effect::{exec::ExecCommand, read_file::ReadFile};

use super::{config::FinalSupergraphConfig, version::SupergraphVersion};

#[derive(thiserror::Error, Debug)]
pub enum RunCompositionError {
    #[error("Failed to run the composition binary")]
    Binary { error: Box<dyn Debug> },
    #[error("Failed to parse output of `{binary} compose`")]
    InvalidOutput {
        binary: Utf8PathBuf,
        error: Utf8Error,
    },
    #[error("Failed to read the file at: {path}")]
    ReadFile {
        path: Utf8PathBuf,
        error: Box<dyn Debug>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OutputTarget {
    File(Utf8PathBuf),
    Stdout,
}

impl OutputTarget {
    pub fn align_to_version(self, version: &SupergraphVersion) -> OutputTarget {
        match self {
            OutputTarget::File(path) => {
                if version.supports_output_flag() {
                    OutputTarget::File(path)
                } else {
                    tracing::warn!("This version of supergraph does not support the `--output flag`. Defaulting to `stdout`");
                    OutputTarget::Stdout
                }
            }
            OutputTarget::Stdout => OutputTarget::Stdout,
        }
    }
}

pub struct SupergraphBinary {
    exe: Utf8PathBuf,
    version: SupergraphVersion,
}

impl SupergraphBinary {
    pub async fn run(
        &self,
        exec: &impl ExecCommand,
        read_file: &impl ReadFile,
        supergraph_config: FinalSupergraphConfig,
        output_target: OutputTarget,
    ) -> Result<String, RunCompositionError> {
        let output_target = output_target.align_to_version(&self.version);
        let mut args = vec!["compose", supergraph_config.path().as_ref()];
        if let OutputTarget::File(output_path) = &output_target {
            args.push(output_path.as_ref());
        }
        let output = exec
            .exec_command(&self.exe, &args)
            .await
            .tap_err(|err| tracing::error!("{:?}", err))
            .map_err(|err| RunCompositionError::Binary {
                error: Box::new(err),
            })?;
        let output =
            match &output_target {
                OutputTarget::File(path) => read_file.read_file(path).await.map_err(|err| {
                    RunCompositionError::ReadFile {
                        path: path.clone(),
                        error: Box::new(err),
                    }
                })?,
                OutputTarget::Stdout => std::str::from_utf8(&output.stdout)
                    .map_err(|err| RunCompositionError::InvalidOutput {
                        binary: self.exe.clone(),
                        error: err,
                    })?
                    .to_string(),
            };
        Ok(output)
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
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use crate::{
        composition::supergraph::{config::FinalSupergraphConfig, version::SupergraphVersion},
        utils::effect::{exec::MockExecCommand, read_file::MockReadFile},
    };

    use super::{OutputTarget, SupergraphBinary};

    fn fed_one() -> Version {
        Version::from_str("1.0.0").unwrap()
    }

    fn fed_two_eight() -> Version {
        Version::from_str("2.8.0").unwrap()
    }

    fn fed_two_nine() -> Version {
        Version::from_str("2.9.0").unwrap()
    }

    #[rstest]
    #[case::fed_one(fed_one(), OutputTarget::Stdout)]
    #[case::fed_one(fed_two_eight(), OutputTarget::Stdout)]
    #[case::fed_one(fed_two_nine(), OutputTarget::File(Utf8PathBuf::new()))]
    fn test_output_target_file_align_to_version(
        #[case] federation_version: Version,
        #[case] expected_output_target: OutputTarget,
    ) {
        let supergraph_version = SupergraphVersion::new(federation_version);
        let given_output_target = OutputTarget::File(Utf8PathBuf::new());
        let result_output_target = given_output_target.align_to_version(&supergraph_version);
        assert_that!(result_output_target).is_equal_to(expected_output_target);
    }

    #[rstest]
    #[case::fed_one(fed_one(), OutputTarget::Stdout)]
    #[case::fed_one(fed_two_eight(), OutputTarget::Stdout)]
    #[case::fed_one(fed_two_nine(), OutputTarget::Stdout)]
    fn test_output_target_stdout_align_to_version(
        #[case] federation_version: Version,
        #[case] expected_output_target: OutputTarget,
    ) {
        let supergraph_version = SupergraphVersion::new(federation_version);
        let given_output_target = OutputTarget::Stdout;
        let result_output_target = given_output_target.align_to_version(&supergraph_version);
        assert_that!(result_output_target).is_equal_to(expected_output_target);
    }

    #[tokio::test]
    async fn test_binary_stdout_output() -> Result<()> {
        let supergraph_version = SupergraphVersion::new(fed_two_eight());
        let binary_path = Utf8PathBuf::from_str("/tmp/supergraph")?;

        let supergraph_binary = SupergraphBinary {
            exe: binary_path.clone(),
            version: supergraph_version,
        };

        let supergraph_config_path = Utf8PathBuf::from_str("/tmp/supergraph_config.yaml")?;
        let supergraph_config = FinalSupergraphConfig::new(
            supergraph_config_path,
            SupergraphConfig::new(BTreeMap::new(), None),
        );
        let output_target = OutputTarget::Stdout;

        let mut mock_read_file = MockReadFile::new();
        mock_read_file.expect_read_file().times(0);
        let mut mock_exec = MockExecCommand::new();
        mock_exec
            .expect_exec_command()
            .times(1)
            .withf(move |actual_binary_path, actual_arguments| {
                actual_binary_path == &binary_path.clone()
                    && actual_arguments == ["compose", "/tmp/supergraph_config.yaml"]
            })
            .returning(|_, _| {
                let stdout = "yes".as_bytes();
                Ok(Output {
                    status: ExitStatus::default(),
                    stdout: stdout.into(),
                    stderr: Vec::default(),
                })
            });
        let result = supergraph_binary
            .run(
                &mock_exec,
                &mock_read_file,
                supergraph_config,
                output_target,
            )
            .await;
        assert_that!(result).is_ok().is_equal_to(&"yes".to_string());
        Ok(())
    }
}
