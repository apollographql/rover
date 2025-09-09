use std::process::Stdio;
use std::{fmt::Debug, path::PathBuf};

use apollo_federation_types::{
    config::FederationVersion,
    rover::{BuildErrors, BuildOutput, BuildResult},
};
use buildstructor::Builder;
use camino::Utf8PathBuf;
use tap::TapFallible;

use super::version::SupergraphVersion;
use crate::RoverOutput;
use crate::command::connector::run::{RunConnectorError, RunConnectorOutput};
use crate::command::connector::test::TestConnectorError;
use crate::utils::effect::exec::ExecCommandOutput;
use crate::{
    composition::{CompositionError, CompositionSuccess},
    utils::effect::exec::{ExecCommand, ExecCommandConfig},
};

impl From<std::io::Error> for CompositionError {
    fn from(error: std::io::Error) -> Self {
        CompositionError::Binary {
            error: error.to_string(),
        }
    }
}

#[derive(Builder, Debug, Clone, derive_getters::Getters)]
pub struct SupergraphBinary {
    exe: Utf8PathBuf,
    version: SupergraphVersion,
}

impl SupergraphBinary {
    pub async fn compose(
        &self,
        exec_impl: &impl ExecCommand,
        supergraph_config_path: Utf8PathBuf,
    ) -> Result<CompositionSuccess, CompositionError> {
        let args = vec!["compose".to_string(), supergraph_config_path.to_string()];

        let config = ExecCommandConfig::builder()
            .exe(self.exe.clone())
            .args(args)
            .output(ExecCommandOutput::builder().stdout(Stdio::piped()).build())
            .build();

        let output = exec_impl
            .exec_command(config)
            .await
            .tap_err(|err| tracing::error!("{:?}", err))
            .map_err(|err| CompositionError::Binary {
                error: format!("{err:?}"),
            })?;

        let exit_code = output.status.code();
        if exit_code != Some(0) && exit_code != Some(1) {
            return Err(CompositionError::BinaryExit {
                exit_code,
                stdout: String::from_utf8(output.stdout).unwrap(),
                stderr: String::from_utf8(output.stderr).unwrap(),
            });
        }

        let output = std::str::from_utf8(&output.stdout)
            .map_err(|err| CompositionError::InvalidOutput {
                binary: self.exe.clone(),
                error: format!("{err:?}"),
            })?
            .to_string();

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
            error: format!("{err:?}"),
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
                federation_version: federation_version.clone(),
            })
            .map_err(|build_errors| CompositionError::Build {
                source: build_errors,
                federation_version,
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
                error: format!("{err:?}"),
            })
    }

    pub async fn run_connector(
        &self,
        exec_impl: &impl ExecCommand,
        schema_path: PathBuf,
        connector_id: String,
        variables: String,
    ) -> Result<RoverOutput, RunConnectorError> {
        let args = vec![
            "run-connector".to_string(),
            "--path".to_string(),
            schema_path.to_str().unwrap_or_default().into(),
            "--connector-id".to_string(),
            connector_id,
            "--variables".to_string(),
            variables,
        ];

        let config = ExecCommandConfig::builder()
            .exe(self.exe.clone())
            .args(args)
            .output(ExecCommandOutput::builder().stdout(Stdio::piped()).build())
            .build();

        let output = exec_impl
            .exec_command(config)
            .await
            .tap_err(|err| tracing::error!("{:?}", err))
            .map_err(|err| RunConnectorError::Binary {
                error: format!("{err:?}"),
            })?;

        let exit_code = output.status.code();
        if exit_code != Some(0) && exit_code != Some(1) {
            return Err(RunConnectorError::BinaryExit {
                exit_code,
                stdout: String::from_utf8(output.stdout).unwrap(),
                stderr: String::from_utf8(output.stderr).unwrap(),
            });
        }

        let output = std::str::from_utf8(&output.stdout)
            .map_err(|err| RunConnectorError::InvalidOutput {
                binary: self.exe.clone(),
                error: format!("{err:?}"),
            })?
            .to_string();

        let output: RunConnectorOutput =
            serde_json::from_str(&output).map_err(|err| RunConnectorError::InvalidOutput {
                binary: self.exe.clone(),
                error: format!("{err:?}"),
            })?;

        Ok(RoverOutput::ConnectorRunResponse { output })
    }

    #[expect(clippy::too_many_arguments)]
    pub async fn test_connector(
        &self,
        exec_impl: &impl ExecCommand,
        file: Option<PathBuf>,
        directory: Option<PathBuf>,
        no_fail: bool,
        schema_file: Option<String>,
        output_file: Option<Utf8PathBuf>,
        verbose: bool,
        quiet: bool,
    ) -> Result<RoverOutput, TestConnectorError> {
        let mut args = vec!["test-connectors".to_string()];

        if no_fail {
            args.push("--no-fail-fast".to_string());
        }

        if verbose {
            args.push("--verbose".to_string());
        }

        if quiet {
            args.push("--quiet".to_string());
        }

        if let Some(file) = file {
            args.push("--file".to_string());
            args.push(file.to_str().unwrap_or_default().to_string());
        }

        if let Some(directory) = directory {
            args.push("--directory".to_string());
            args.push(directory.to_str().unwrap_or_default().to_string());
        }

        if let Some(output_file) = output_file {
            args.push("--report".to_string());
            args.push(output_file.into_string());
        }

        if let Some(schema_file) = schema_file {
            args.push("--schema-file".to_string());
            args.push(schema_file);
        }

        let config = ExecCommandConfig::builder()
            .exe(self.exe.clone())
            .args(args)
            .output(ExecCommandOutput::builder().stdout(Stdio::piped()).build())
            .build();

        let output = exec_impl
            .exec_command(config)
            .await
            .tap_err(|err| tracing::error!("{:?}", err))
            .map_err(|err| TestConnectorError::Binary {
                error: format!("{err:?}"),
            })?;

        let exit_code = output.status.code();
        if exit_code != Some(0) && exit_code != Some(1) {
            return Err(TestConnectorError::BinaryExit {
                exit_code,
                stdout: String::from_utf8(output.stdout).unwrap(),
                stderr: String::from_utf8(output.stderr).unwrap(),
            });
        }

        let output = std::str::from_utf8(&output.stdout)
            .map_err(|err| TestConnectorError::InvalidOutput {
                binary: self.exe.clone(),
                error: format!("{err:?}"),
            })?
            .to_string();

        Ok(RoverOutput::ConnectorTestResponse { output })
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
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use houston::Config;
    use rstest::{fixture, rstest};
    use semver::Version;
    use speculoos::prelude::*;

    use super::{CompositionSuccess, SupergraphBinary};
    use crate::{
        command::supergraph::compose::do_compose::SupergraphComposeOpts,
        composition::{supergraph::version::SupergraphVersion, test::default_composition_json},
        utils::{
            client::{ClientBuilder, ClientTimeout, StudioClientConfig},
            effect::exec::MockExecCommand,
        },
    };

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
    #[once]
    fn client_config() -> StudioClientConfig {
        let home = TempDir::new().unwrap();
        let config = Config {
            home: Utf8PathBuf::from_path_buf(home.path().to_path_buf()).unwrap(),
            override_api_key: None,
        };
        StudioClientConfig::new(
            None,
            config,
            false,
            ClientBuilder::default(),
            ClientTimeout::default(),
        )
    }

    fn composition_output(version: Version) -> CompositionSuccess {
        let res = build_result().unwrap();

        CompositionSuccess {
            hints: res.hints,
            supergraph_sdl: res.supergraph_sdl,
            federation_version: FederationVersion::ExactFedTwo(version),
        }
    }

    #[rstest]
    #[tokio::test]
    async fn test_compose_success() -> Result<()> {
        let version = fed_two_nine();
        let composition_output = composition_output(version.clone());
        let supergraph_version = SupergraphVersion::new(version);
        let binary_path = Utf8PathBuf::from_str("/supergraph")?;
        let supergraph_binary = SupergraphBinary::builder()
            .exe(binary_path.clone())
            .version(supergraph_version)
            .build();

        let mut opts = SupergraphComposeOpts::default();
        opts.plugin_opts.elv2_license_accepter.elv2_license_accepted = Some(true);

        let temp_supergraph_config_path = Utf8PathBuf::from_str("/supergraph_config.yaml")?;

        let mut mock_exec = MockExecCommand::new();

        mock_exec
            .expect_exec_command()
            .times(1)
            .withf(move |actual_config| {
                let expected_args =
                    vec!["compose".to_string(), "/supergraph_config.yaml".to_string()];
                actual_config.exe() == &binary_path && actual_config.args() == &Some(expected_args)
            })
            .returning(move |_| {
                let stdout = serde_json::to_string(&default_composition_json()).unwrap();
                Ok(Output {
                    status: ExitStatus::default(),
                    stdout: stdout.as_bytes().into(),
                    stderr: Vec::default(),
                })
            });

        let result = supergraph_binary
            .compose(&mock_exec, temp_supergraph_config_path)
            .await;

        assert_that!(result).is_ok().is_equal_to(composition_output);

        Ok(())
    }
}
