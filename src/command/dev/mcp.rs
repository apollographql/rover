use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;
use strum_macros::Display;
use tracing::Level;

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
    directory: Option<Utf8PathBuf>,

    /// Start the MCP server using the Streamable HTTP transport on the given IP address
    #[arg(
        long = "mcp-address",
        alias = "mcp-sse-address",
        default_value = "127.0.0.1"
    )]
    address: String,

    /// Start the MCP server using the Streamable HTTP transport on the given port
    #[arg(long = "mcp-port", alias = "mcp-sse-port", default_value = "5000")]
    port: u16,

    /// Expose the schema to the MCP client through `schema` and `execute` tools - defaults to true
    #[arg(long = "mcp-introspection")]
    introspection: bool,

    /// Operation files to expose as MCP tools
    #[arg(long = "mcp-operations", num_args=0..)]
    operations: Vec<Utf8PathBuf>,

    /// Headers to send to the endpoint
    #[arg(long = "mcp-header", action = clap::ArgAction::Append)]
    headers: Vec<String>,

    /// The path to the persisted query manifest containing operations
    #[arg(long = "mcp-manifest")]
    manifest: Option<Utf8PathBuf>,

    /// collection id to expose as MCP tools (requires APOLLO_KEY)
    #[arg(long = "mcp-collection")]
    collection: Option<String>,

    /// Enable use of uplink to get persisted queries (requires APOLLO_KEY and APOLLO_GRAPH_REF)
    #[arg(long = "mcp-uplink-manifest")]
    uplink_manifest: bool,

    /// The path to the GraphQL custom_scalars_config file
    #[arg(long = "mcp-custom-scalars-config", required = false)]
    custom_scalars_config: Option<Utf8PathBuf>,

    // Configure when to allow mutations
    #[arg(long = "mcp-allow-mutations", default_value_t, value_enum)]
    allow_mutations: MutationMode,

    /// Disable operation root field types in tool description
    #[arg(long = "mcp-disable-type-description")]
    disable_type_description: bool,

    /// Disable schema type definitions referenced by all fields returned by the operation in the tool description
    #[arg(long = "mcp-disable-schema-description")]
    disable_schema_description: bool,

    /// Expose a tool that returns the URL to open a GraphQL operation in Apollo Explorer (requires APOLLO_GRAPH_REF)
    #[arg(long = "mcp-explorer")]
    explorer: bool,

    /// Change the level at which the MCP Server logs
    #[arg(long = "mcp-log", default_value_t = Level::INFO)]
    #[serde(skip)]
    log_level: Level,
}
