use std::path::PathBuf;

use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

use crate::composition::supergraph::binary::SupergraphBinary;
use crate::utils::effect::exec::TokioCommand;
use crate::{RoverOutput, RoverResult};

#[derive(thiserror::Error, Debug)]
pub enum TestConnectorError {
    #[error("Failed to run the connectors binary")]
    Binary { error: String },

    #[error("The connectors binary exited with errors.\nStdout: {}\nStderr: {}", .stdout, .stderr)]
    BinaryExit {
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },

    #[error("Failed to parse output of `{binary} connectors test`\n{error}")]
    InvalidOutput { binary: Utf8PathBuf, error: String },
}

#[derive(Debug, Parser, Clone, Serialize)]
pub struct TestConnector {
    /// Defines a single test suite file source
    /// If no directory and no file is passed, it will default to `--directory tests/`
    #[arg(short = 'f', long = "file")]
    file: Option<PathBuf>,

    /// Defines a test suite directory, will look for any file ending in `.connector.yml`.
    /// If no directory and no file is passed, it will default to `--directory tests/`
    #[arg(short = 'd', long = "directory")]
    directory: Option<PathBuf>,

    /// Avoids failure on asserting error, only logging test error
    #[arg(long = "no-fail-fast", default_value = "false")]
    no_fail: bool,

    /// Schema file to override `config.schema` (or missing schema fields) for all test suites
    #[arg(long = "schema-file")]
    schema_file: Option<String>,

    /// JUnit XML Report output location
    #[arg(long = "report")]
    output: Option<PathBuf>,

    // TODO: Remove after logging config has been integrated
    /// Hides test progression. Defaults to 'false'
    #[arg(long = "quiet", short = 'q', default_value = "false")]
    pub quiet: bool,

    // TODO: Remove after logging config has been integrated
    /// Enable verbose logging. Defaults to 'false'.
    #[arg(long = "verbose", short = 'v')]
    pub verbose: bool,
}

impl TestConnector {
    pub async fn run(&self, supergraph_binary: SupergraphBinary) -> RoverResult<RoverOutput> {
        let exec_command_impl = TokioCommand::default();
        let result = supergraph_binary
            .test_connector(
                &exec_command_impl,
                self.file.clone(),
                self.directory.clone(),
                self.no_fail,
                self.schema_file.clone(),
                self.output
                    .as_ref()
                    .and_then(|path| camino::Utf8PathBuf::from_path_buf(path.to_path_buf()).ok()),
                self.verbose,
                self.quiet,
            )
            .await?;
        Ok(result)
    }
}
