use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

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
}
