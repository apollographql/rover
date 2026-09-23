use std::str::FromStr;

use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

use crate::{command::install::McpServerVersion, plugin::error::RequestOrigin};

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

impl Opts {
    /// Where `version` came from. Clap folds the flag and its environment
    /// variable into one value, and the flag wins when both are set.
    pub(crate) fn version_origin(&self) -> Option<RequestOrigin> {
        self.version.as_ref()?;
        let flag_given =
            std::env::args().any(|arg| arg == "--mcp-version" || arg.starts_with("--mcp-version="));
        let from_env = !flag_given && std::env::var_os(MCP_VERSION_ENV).is_some();
        Some(if from_env {
            RequestOrigin::EnvVar(MCP_VERSION_ENV)
        } else {
            RequestOrigin::Flag("--mcp-version")
        })
    }
}

const MCP_VERSION_ENV: &str = "APOLLO_ROVER_DEV_MCP_VERSION";

#[cfg(test)]
mod version_origin_tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    // The test binary's own arguments never include `--mcp-version`, so these
    // cover a version set without the flag.
    #[rstest]
    #[case::no_version(None, Some("1.0.0"), None)]
    #[case::the_env_var(
        Some("=1.0.0"),
        Some("1.0.0"),
        Some(RequestOrigin::EnvVar(MCP_VERSION_ENV))
    )]
    #[case::no_env_var(Some("=1.0.0"), None, Some(RequestOrigin::Flag("--mcp-version")))]
    fn the_version_names_where_it_came_from(
        #[case] version: Option<&str>,
        #[case] env: Option<&str>,
        #[case] expected: Option<RequestOrigin>,
    ) {
        let opts = Opts {
            config: None,
            version: version.map(|version| McpServerVersion::from_str(version).unwrap()),
        };

        let origin = temp_env::with_var(MCP_VERSION_ENV, env, || opts.version_origin());

        assert_that!(origin).is_equal_to(expected);
    }
}
