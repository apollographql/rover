use std::fmt::Debug;

use apollo_federation_types::{
    config::FederationVersion,
    rover::{BuildErrors, BuildOutput, BuildResult},
};
use camino::Utf8PathBuf;
use tap::TapFallible;

use crate::{
    composition::{CompositionError, CompositionSuccess},
    utils::effect::{exec::ExecCommand, read_file::ReadFile},
};

use super::version::SupergraphVersion;

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

impl From<std::io::Error> for CompositionError {
    fn from(error: std::io::Error) -> Self {
        CompositionError::Binary {
            error: error.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct SupergraphBinary {
    exe: Utf8PathBuf,
    version: SupergraphVersion,
    output_target: OutputTarget,
}

impl SupergraphBinary {
    pub fn new(exe: Utf8PathBuf, version: SupergraphVersion, output_target: OutputTarget) -> Self {
        Self {
            exe,
            version: version.clone(),
            output_target: output_target.align_to_version(&version),
        }
    }

    fn prepare_compose_args(&self, supergraph_config_path: &Utf8PathBuf) -> Vec<String> {
        let mut args = vec!["compose".to_string(), supergraph_config_path.to_string()];

        if let OutputTarget::File(output_path) = &self.output_target {
            args.push(output_path.to_string());
        }

        args
    }

    pub async fn compose(
        &self,
        exec_impl: &impl ExecCommand,
        read_file_impl: &impl ReadFile,
        supergraph_config_path: Utf8PathBuf,
    ) -> Result<CompositionSuccess, CompositionError> {
        let args = self.prepare_compose_args(&supergraph_config_path);

        let output = exec_impl
            .exec_command(&self.exe, &args)
            .await
            .tap_err(|err| tracing::error!("{:?}", err))
            .map_err(|err| CompositionError::Binary {
                error: format!("{:?}", err),
            })?;

        let output = match &self.output_target {
            OutputTarget::File(path) => {
                read_file_impl
                    .read_file(path)
                    .await
                    .map_err(|err| CompositionError::ReadFile {
                        path: path.clone(),
                        error: format!("{:?}", err),
                    })?
            }
            OutputTarget::Stdout => std::str::from_utf8(&output.stdout)
                .map_err(|err| CompositionError::InvalidOutput {
                    binary: self.exe.clone(),
                    error: format!("{:?}", err),
                })?
                .to_string(),
        };

        self.validate_composition(&output)
    }

    /// Validate that the output of the supergraph binary contains either build errors or build
    /// output, which we'll use later when validating that we have a well-formed composition
    fn validate_supergraph_binary_output(
        &self,
        output: &str,
    ) -> Result<Result<BuildOutput, BuildErrors>, CompositionError> {
        // Attempt to convert the str to a valid composition result; this ensures that we have a
        // well-formed composition. This doesn't necessarily mean we don't have build errors, but
        // we handle those below
        serde_json::from_str::<BuildResult>(output).map_err(|err| CompositionError::InvalidOutput {
            binary: self.exe.clone(),
            error: format!("{:?}", err),
        })
    }

    /// Validates both that the supergraph binary produced a useable output and that that output
    /// represents a valid composition (even if it results in build errors)
    fn validate_composition(
        &self,
        supergraph_binary_output: &str,
    ) -> Result<CompositionSuccess, CompositionError> {
        // Validate the supergraph version is a supported federation version
        let federation_version = self.federation_version()?;

        self.validate_supergraph_binary_output(supergraph_binary_output)?
            .map(|build_output| CompositionSuccess {
                hints: build_output.hints,
                supergraph_sdl: build_output.supergraph_sdl,
                federation_version,
            })
            .map_err(|build_errors| CompositionError::Build {
                source: build_errors,
            })
    }

    /// Using the supergraph binary's version to get the supported Federation version
    ///
    /// At the time of writing, these versions are the same. That is, a supergraph binary version
    /// just is the supported Federation version
    fn federation_version(&self) -> Result<FederationVersion, CompositionError> {
        self.version
            .clone()
            .try_into()
            .map_err(|err| CompositionError::InvalidInput {
                binary: self.exe.clone(),
                error: format!("{:?}", err),
            })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        process::{ExitStatus, Output},
        str::FromStr,
    };

    use anyhow::Result;
    use apollo_federation_types::{config::FederationVersion, rover::BuildResult};
    use camino::Utf8PathBuf;
    use rstest::{fixture, rstest};
    use semver::Version;
    use speculoos::prelude::*;

    use crate::{
        composition::{supergraph::version::SupergraphVersion, test::default_composition_json},
        utils::effect::{exec::MockExecCommand, read_file::MockReadFile},
    };

    use super::{CompositionSuccess, OutputTarget, SupergraphBinary};

    fn fed_one() -> Version {
        Version::from_str("1.0.0").unwrap()
    }

    fn fed_two_eight() -> Version {
        Version::from_str("2.8.0").unwrap()
    }

    fn fed_two_nine() -> Version {
        Version::from_str("2.9.0").unwrap()
    }

    #[fixture]
    fn supergraph_config_path() -> Utf8PathBuf {
        Utf8PathBuf::from_str("dummy_supergraph_config_path").unwrap()
    }

    #[fixture]
    fn build_result() -> BuildResult {
        serde_json::from_value(default_composition_json()).unwrap()
    }

    #[fixture]
    fn composition_output() -> CompositionSuccess {
        let res = build_result().unwrap();

        CompositionSuccess {
            hints: res.hints,
            supergraph_sdl: res.supergraph_sdl,
            federation_version: FederationVersion::ExactFedTwo(fed_two_eight()),
        }
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
    #[case::fed_two_eight(fed_two_eight(), OutputTarget::Stdout)]
    #[case::fed_two_nine(fed_two_nine(), OutputTarget::Stdout)]
    fn test_output_target_stdout_align_to_version(
        #[case] federation_version: Version,
        #[case] expected_output_target: OutputTarget,
    ) {
        let supergraph_version = SupergraphVersion::new(federation_version);
        let given_output_target = OutputTarget::Stdout;
        let result_output_target = given_output_target.align_to_version(&supergraph_version);
        assert_that!(result_output_target).is_equal_to(expected_output_target);
    }

    #[rstest]
    #[case::with_output_path(OutputTarget::File(Utf8PathBuf::from_str("some_output_file").unwrap()), vec!["compose", "dummy_supergraph_config_path", "some_output_file"])]
    #[case::without_output_path(OutputTarget::Stdout, vec!["compose", "dummy_supergraph_config_path"])]
    fn test_prepare_compose_args(
        #[case] test_output_target: OutputTarget,
        #[case] expected_args: Vec<&str>,
        supergraph_config_path: Utf8PathBuf,
    ) {
        let supergraph_version = SupergraphVersion::new(fed_two_eight());
        let binary_path = Utf8PathBuf::from_str("/tmp/supergraph").unwrap();
        //let output_target = OutputTarget::Stdout;

        let supergraph_binary = SupergraphBinary {
            exe: binary_path.clone(),
            version: supergraph_version,
            output_target: test_output_target,
        };

        let args = supergraph_binary.prepare_compose_args(&supergraph_config_path);

        assert_eq!(args, expected_args);
    }

    #[rstest]
    #[tokio::test]
    async fn test_compose_success(composition_output: CompositionSuccess) -> Result<()> {
        let supergraph_version = SupergraphVersion::new(fed_two_eight());
        let binary_path = Utf8PathBuf::from_str("/tmp/supergraph")?;
        let output_target = OutputTarget::Stdout;

        let supergraph_binary = SupergraphBinary {
            exe: binary_path.clone(),
            version: supergraph_version,
            output_target,
        };

        let temp_supergraph_config_path =
            Utf8PathBuf::from_str("/tmp/target/supergraph_config.yaml")?;

        let mut mock_read_file = MockReadFile::new();
        mock_read_file.expect_read_file().times(0);
        let mut mock_exec = MockExecCommand::new();

        mock_exec
            .expect_exec_command()
            .times(1)
            .withf(move |actual_binary_path, actual_arguments| {
                actual_binary_path == &binary_path.clone()
                    && actual_arguments == ["compose", "/tmp/target/supergraph_config.yaml"]
            })
            .returning(move |_, _| {
                let stdout = serde_json::to_string(&default_composition_json()).unwrap();
                Ok(Output {
                    status: ExitStatus::default(),
                    stdout: stdout.as_bytes().into(),
                    stderr: Vec::default(),
                })
            });

        let result = supergraph_binary
            .compose(&mock_exec, &mock_read_file, temp_supergraph_config_path)
            .await;
        assert_that!(result).is_ok().is_equal_to(composition_output);

        Ok(())
    }
}
