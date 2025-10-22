use std::str::FromStr;

use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

use crate::command::install::McpServerVersion;

pub mod binary;
pub mod install;
pub mod run;

fn parse_mcp_version(s: &str) -> Result<McpServerVersion, String> {
    // Add the '=' prefix if not already present, as McpServerVersion expects it
    let prefixed = if s.starts_with('=') || s == "latest" {
        s.to_string()
    } else {
        format!("={}", s)
    };
    McpServerVersion::from_str(&prefixed).map_err(|e| e.to_string())
}

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
    #[arg(long = "mcp-version", env = "APOLLO_ROVER_DEV_MCP_VERSION", value_parser = parse_mcp_version)]
    pub version: Option<McpServerVersion>,
}
