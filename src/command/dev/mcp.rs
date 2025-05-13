use std::path::PathBuf;

use clap::Parser;
use serde::Serialize;
use strum_macros::Display;

pub mod binary;
pub mod install;
pub mod run;

#[derive(clap::ValueEnum, Clone, Default, Debug, Display, Serialize)]
enum MutationMode {
    /// Don't allow any mutations
    #[default]
    None,
    /// Allow explicit mutations, but don't allow the LLM to build them
    Explicit,
    /// Allow the LLM to build mutations
    All,
}

#[derive(Debug, Clone, Serialize, Parser)]
pub struct Opts {
    /// Enable the MCP server
    #[arg(long = "mcp")]
    pub enabled: bool,

    /// The working directory to use
    #[arg(long = "mcp-directory", required = false)]
    directory: Option<PathBuf>,

    /// Start the server using the SSE transport on the given port
    #[arg(long = "mcp-sse-port", default_value = "5000")]
    sse_port: u16,

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

    /// The path to the GraphQL custom_scalars_config file
    #[arg(long = "mcp-custom-scalars-config", required = false)]
    custom_scalars_config: Option<PathBuf>,

    // Configure when to allow mutations
    #[arg(long = "mcp-allow-mutations", default_value_t, value_enum)]
    allow_mutations: MutationMode,
}
