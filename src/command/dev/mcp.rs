use std::path::PathBuf;

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

    /// Start the server using the SSE transport on the given port
    #[arg(long = "mcp-port")]
    port: Option<u16>,

    /// Expose the schema to the MCP client through `schema` and `execute` tools - defaults to true
    #[arg(long = "mcp-introspection")]
    introspection: bool,

    /// Operation files to expose as MCP tools
    #[arg(long = "mcp-operations", num_args=0..)]
    operations: Vec<PathBuf>,

    /// Headers to send to the endpoint
    #[arg(long = "mcp-header", action = clap::ArgAction::Append)]
    headers: Vec<String>,

    /// The path to the persisted query manifest containing operations
    #[arg(long = "mcp-manifest")]
    manifest: Option<PathBuf>,
}
