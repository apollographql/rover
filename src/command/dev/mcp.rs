use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

use crate::command::install::McpServerVersion;

pub mod binary;
pub mod install;
pub mod run;

#[derive(Debug, Clone, Serialize, Parser)]
pub struct Opts {
    /// Enable the MCP server and (optionally) specify the path to the config file
    ///
    /// Note: This uses default options if omitted
    #[arg(long = "mcp", default_missing_value = None)]
    pub config: Option<Option<Utf8PathBuf>>,

    /// The version of the MCP server to use
    ///
    /// You can also use the `APOLLO_ROVER_DEV_MCP_VERSION` environment variable
    #[arg(long = "mcp-version", env = "APOLLO_ROVER_DEV_MCP_VERSION")]
    pub version: Option<McpServerVersion>,
}
