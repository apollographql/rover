use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

pub mod binary;
pub mod install;
pub mod run;

#[derive(Debug, Clone, Serialize, Parser)]
pub struct Opts {
    /// Enable the MCP server
    #[arg(long = "mcp")]
    pub enabled: bool,

    /// The path to the MCP config file
    #[arg(long = "mcp-config")]
    config: Option<Utf8PathBuf>,
}
