use std::path::PathBuf;

use clap::Parser;
use serde::Serialize;

use crate::composition::supergraph::binary::SupergraphBinary;
use crate::utils::effect::exec::TokioCommand;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Parser, Clone, Serialize)]
pub struct GenerateConnector {
    /// Sets the name of the generate file (`<name>.graphql` will be generated file`).
    ///
    /// Defaults to `output`
    #[clap(short, long, value_name = "NAME")]
    name: Option<String>,

    /// Set analysis directory to load data from.
    ///
    /// Defaults to `$(pwd)/analysis/`
    #[clap(short, long, value_name = "ANALYSIS_DIR")]
    analysis_dir: Option<PathBuf>,

    /// Set a custom directory to generate output files to.
    ///
    /// Defaults to `build/connectors/`.
    #[clap(long, value_name = "OUTPUT_DIR")]
    output_dir: Option<PathBuf>,
}

impl GenerateConnector {
    pub async fn run(&self, supergraph_binary: SupergraphBinary) -> RoverResult<RoverOutput> {
        let exec_command_impl = TokioCommand::default();
        let result = supergraph_binary
            .generate_connector(
                &exec_command_impl,
                self.name.clone(),
                self.analysis_dir
                    .as_ref()
                    .and_then(|path| camino::Utf8PathBuf::from_path_buf(path.to_path_buf()).ok()),
                self.output_dir
                    .as_ref()
                    .and_then(|path| camino::Utf8PathBuf::from_path_buf(path.to_path_buf()).ok()),
            )
            .await?;
        Ok(result)
    }
}
