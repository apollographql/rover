use std::path::PathBuf;

use clap::Parser;
use serde::Serialize;

use crate::composition::supergraph::binary::SupergraphBinary;
use crate::utils::effect::exec::TokioCommand;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Parser, Clone, Serialize)]
pub struct ListConnector {
    #[arg(long, value_name = "SCHEMA_PATH")]
    schema_path: PathBuf,
}

impl ListConnector {
    pub async fn run(&self, supergraph_binary: SupergraphBinary) -> RoverResult<RoverOutput> {
        let exec_command_impl = TokioCommand::default();
        let result = supergraph_binary
            .list_connector(
                &exec_command_impl,
                camino::Utf8PathBuf::from_path_buf(self.schema_path.to_path_buf())
                    .unwrap_or_default(),
            )
            .await?;
        Ok(result)
    }
}
