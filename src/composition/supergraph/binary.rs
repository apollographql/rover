use std::fmt::Debug;

use camino::Utf8PathBuf;
use derive_getters::Getters;
use tap::TapFallible;

use crate::utils::effect::{exec::ExecCommand, read_file::ReadFile};

use apollo_federation_types::{
    build::{BuildErrors, BuildHint, BuildOutput, BuildResult},
    config::FederationVersion,
};

use super::{config::ResolvedSupergraphConfig, version::SupergraphVersion};

#[derive(thiserror::Error, Debug)]
pub enum CompositionError {
    #[error("Failed to run the composition binary")]
    Binary { error: Box<dyn Debug> },
    #[error("Failed to parse output of `{binary} compose`")]
    InvalidOutput {
        binary: Utf8PathBuf,
        error: Box<dyn Debug>,
    },
    #[error("Invalid input for `{binary} compose`")]
    InvalidInput {
        binary: Utf8PathBuf,
        error: Box<dyn Debug>,
    },
    #[error("Failed to read the file at: {path}")]
    ReadFile {
        path: Utf8PathBuf,
        error: Box<dyn Debug>,
    },
    #[error("Encountered {} while trying to build a supergraph.", .source.length_string())]
    Build {
        source: BuildErrors,
        // NB: in do_compose (rover_client/src/error -> BuildErrors) this includes num_subgraphs,
        // but this is only important if we end up with a RoverError (it uses a singular or plural
        // error message); so, leaving TBD if we go that route because it'll require figuring out
        // from something like the supergraph_config how many subgraphs we attempted to compose
        // (alternatively, we could just reword the error message to allow for either)
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

#[derive(Getters, Debug, Clone, Eq, PartialEq)]
pub struct CompositionOutput {
    supergraph_sdl: String,
    hints: Vec<BuildHint>,
    federation_version: FederationVersion,
}

impl SupergraphBinary {
    pub async fn compose(
        &self,
        exec: &impl ExecCommand,
        read_file: &impl ReadFile,
        supergraph_config: ResolvedSupergraphConfig,
        output_target: OutputTarget,
    ) -> Result<CompositionOutput, CompositionError> {
        let output_target = output_target.align_to_version(&self.version);
        let mut args = vec!["compose", supergraph_config.path().as_ref()];
        if let OutputTarget::File(output_path) = &output_target {
            args.push(output_path.as_ref());
        }
        let output = exec
            .exec_command(&self.exe, &args)
            .await
            .tap_err(|err| tracing::error!("{:?}", err))
            .map_err(|err| CompositionError::Binary {
                error: Box::new(err),
            })?;
        let output = match &output_target {
            OutputTarget::File(path) => {
                read_file
                    .read_file(path)
                    .await
                    .map_err(|err| CompositionError::ReadFile {
                        path: path.clone(),
                        error: Box::new(err),
                    })?
            }
            OutputTarget::Stdout => std::str::from_utf8(&output.stdout)
                .map_err(|err| CompositionError::InvalidOutput {
                    binary: self.exe.clone(),
                    error: Box::new(err),
                })?
                .to_string(),
        };

        self.validate_composition(&output)
    }

    /// Validate that the output of the supergraph binary contains either build errors or build
    /// output, which we'll use later when validating that we have a well-formed composition
    fn validate_supergraph_binary_output(
        &self,
        output: &String,
    ) -> Result<Result<BuildOutput, BuildErrors>, CompositionError> {
        // Attempt to convert the str to a valid composition result; this ensures that we have a
        // well-formed composition. This doesn't necessarily mean we don't have build errors, but
        // we handle those below
        serde_json::from_str::<BuildResult>(output).map_err(|err| CompositionError::InvalidOutput {
            binary: self.exe.clone(),
            error: Box::new(err),
        })
    }

    /// Validates both that the supergraph binary produced a useable output and that that output
    /// represents a valid composition (even if it results in build errors)
    fn validate_composition(
        &self,
        supergraph_binary_output: &String,
    ) -> Result<CompositionOutput, CompositionError> {
        // Validate the supergraph version is a supported federation version
        let federation_version = self.get_federation_version()?;

        self.validate_supergraph_binary_output(supergraph_binary_output)?
            .map(|build_output| CompositionOutput {
                hints: build_output.hints,
                supergraph_sdl: build_output.supergraph_sdl,
                federation_version,
            })
            .map_err(|build_errors| CompositionError::Build {
                source: build_errors,
            })
    }

    /// Using the supergraph binary's version to get the supported Federation veresion
    ///
    /// At the time of writing, these versions are the same. That is, a supergraph binary version
    /// just is the supported Federation version
    fn get_federation_version(&self) -> Result<FederationVersion, CompositionError> {
        self.version
            .clone()
            .try_into()
            .map_err(|error| CompositionError::InvalidInput {
                binary: self.exe.clone(),
                error: Box::new(error),
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
    use camino::Utf8PathBuf;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use crate::{
        composition::supergraph::{config::ResolvedSupergraphConfig, version::SupergraphVersion},
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
        let supergraph_config = ResolvedSupergraphConfig::load(&supergraph_config_path).await?;
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
            .compose(
                &mock_exec,
                &mock_read_file,
                supergraph_config,
                output_target,
            )
            .await;
        // FIXME: yes
        //assert_that!(result).is_ok().is_equal_to(&"yes".to_string());
        Ok(())
    }
}
