use std::path::PathBuf;

use anyhow::anyhow;
use clap::Parser;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult, composition::supergraph::binary::SupergraphBinary,
    utils::effect::exec::TokioCommand,
};

#[derive(Debug, Parser, Clone, Serialize)]
pub struct ListConnector {
    /// The path to the schema file containing the connector.
    ///
    /// Optional if there is a `supergraph.yaml` containing only a single subgraph
    #[arg(long = "schema", value_name = "SCHEMA_FILE_PATH")]
    schema: Option<PathBuf>,
}

impl ListConnector {
    pub async fn run(
        &self,
        supergraph_binary: SupergraphBinary,
        default_subgraph: Option<PathBuf>,
    ) -> RoverResult<RoverOutput> {
        let exec_command_impl = TokioCommand::default();
        let schema_path = self.schema.clone().or(default_subgraph).ok_or_else(|| anyhow!(
            "A schema path must be provided either via --schema or a `supergraph.yaml` containing a single subgraph"
        ))?;
        
        let result = supergraph_binary
            .list_connector(
                &exec_command_impl,
                camino::Utf8PathBuf::from_path_buf(schema_path?).unwrap_or_default(),
            )
            .await?;
        Ok(result)
    }
}
