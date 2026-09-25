#![warn(missing_docs)]

use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;

#[cfg(feature = "composition-js")]
mod do_dev;
#[cfg(feature = "composition-js")]
mod mcp;
#[cfg(not(feature = "composition-js"))]
mod no_dev;
#[cfg(feature = "composition-js")]
mod router;

use std::net::IpAddr;

use clap::Parser;
use derive_getters::Getters;
use rover_studio::types::GraphRef;
use serde::Serialize;

use crate::{
    options::{OptionalSubgraphOpts, PluginOpts},
    utils::parsers::FileDescriptorType,
};

#[derive(Debug, Serialize, Parser)]
/// Command that represents running a local router, and composition to test local changes to
/// subgraphs.
pub struct Dev {
    #[clap(flatten)]
    pub(crate) opts: DevOpts,
}

#[derive(Debug, Serialize, Parser)]
pub struct DevOpts {
    #[clap(flatten)]
    pub plugin_opts: PluginOpts,

    #[clap(flatten)]
    pub subgraph_opts: OptionalSubgraphOpts,

    #[clap(flatten)]
    pub supergraph_opts: SupergraphOpts,

    #[cfg(feature = "composition-js")]
    #[clap(flatten)]
    pub mcp: mcp::Opts,
}

#[derive(Debug, Parser, Serialize, Clone, Getters)]
pub struct SupergraphOpts {
    /// The port the graph router should listen on.
    ///
    /// If you start multiple `rover dev` processes on the same address and port, they will communicate with each other.
    ///
    /// If you start multiple `rover dev` processes with different addresses and ports, they will not communicate with each other.
    #[arg(long, short = 'p')]
    supergraph_port: Option<u16>,

    /// The address the graph router should listen on.
    ///
    /// If you start multiple `rover dev` processes on the same address and port, they will communicate with each other.
    ///
    /// If you start multiple `rover dev` processes with different addresses and ports, they will not communicate with each other.
    #[arg(long)]
    supergraph_address: Option<IpAddr>,

    /// The path to a router configuration file. If the file path is empty, a default configuration will be written to that file. This file is then watched for changes and propagated to the router.
    ///
    /// For information on the format of this file, please see https://www.apollographql.com/docs/router/configuration/overview/#yaml-config-file.
    #[arg(long = "router-config")]
    #[serde(skip_serializing)]
    router_config_path: Option<Utf8PathBuf>,

    /// The path to a supergraph configuration file. If provided, subgraphs will be loaded from this
    /// file.
    ///
    /// Cannot be used with `--url`, `--name`, or `--schema`.
    ///
    /// For information on the format of this file, please see https://www.apollographql.com/docs/rover/commands/supergraphs/#yaml-configuration-file.
    #[arg(
        long = "supergraph-config",
        conflicts_with_all = ["subgraph_name", "subgraph_url", "subgraph_schema_path"]
    )]
    supergraph_config_path: Option<FileDescriptorType>,

    /// A [`GraphRef`] that is accessible in Apollo Studio.
    /// This is used to initialize your supergraph with the values contained in this variant.
    ///
    /// This is analogous to providing a supergraph.yaml file with references to your graph variant in studio.
    ///
    /// If used in conjunction with `--supergraph-config`, the values presented in the supergraph.yaml will take precedence over these values.
    #[arg(long = "graph-ref")]
    graph_ref: Option<GraphRef>,

    /// The version of Apollo Federation to use for composition
    #[arg(long = "federation-version")]
    federation_version: Option<FederationVersion>,

    /// The path to an offline enterprise license file.
    ///
    /// For more information, please see https://www.apollographql.com/docs/router/enterprise-features/#offline-enterprise-license
    #[arg(long)]
    license: Option<Utf8PathBuf>,

    /// Path to write the composed supergraph schema to, (re)writing it on every successful composition.
    #[arg(long = "supergraph-output")]
    supergraph_output: Option<Utf8PathBuf>,

    /// The version of the GraphOS Router to use.
    ///
    /// You can also use the `APOLLO_ROVER_DEV_ROUTER_VERSION` environment variable.
    #[arg(long = "router-version", env = "APOLLO_ROVER_DEV_ROUTER_VERSION")]
    pub(crate) router_version: Option<String>,

    /// The version of Apollo Federation to use for composition.
    ///
    /// You can also use the `APOLLO_ROVER_DEV_COMPOSITION_VERSION` environment
    /// variable. `--federation-version` takes precedence over both.
    // this number should be mapped to the federation version used by the router
    // https://www.apollographql.com/docs/router/federation-version-support/#support-table
    #[arg(
        long = "composition-version",
        env = "APOLLO_ROVER_DEV_COMPOSITION_VERSION"
    )]
    pub(crate) composition_version: Option<String>,
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use clap::Parser;
    use speculoos::prelude::*;

    use super::DevOpts;

    #[test]
    fn supergraph_output_flag_parses_into_supergraph_opts() {
        let opts =
            DevOpts::try_parse_from(["dev", "--supergraph-output", "build/supergraph.graphql"])
                .unwrap();
        assert_eq!(
            opts.supergraph_opts.supergraph_output,
            Some(Utf8PathBuf::from("build/supergraph.graphql"))
        );
    }

    #[test]
    fn supergraph_output_defaults_to_none() {
        let opts = DevOpts::try_parse_from(["dev"]).unwrap();
        assert_eq!(opts.supergraph_opts.supergraph_output, None);
    }

    #[test]
    fn router_version_flag_wins_over_env_var() {
        let opts = temp_env::with_var("APOLLO_ROVER_DEV_ROUTER_VERSION", Some("1.2.3"), || {
            DevOpts::try_parse_from(["dev", "--router-version", "4.5.6"])
        })
        .unwrap();
        assert_that!(opts.supergraph_opts.router_version).is_equal_to(Some("4.5.6".to_string()));
    }

    #[test]
    fn router_version_env_var_applies_alone() {
        let opts = temp_env::with_var("APOLLO_ROVER_DEV_ROUTER_VERSION", Some("1.2.3"), || {
            DevOpts::try_parse_from(["dev"])
        })
        .unwrap();
        assert_that!(opts.supergraph_opts.router_version).is_equal_to(Some("1.2.3".to_string()));
    }

    #[test]
    fn composition_version_flag_wins_over_env_var() {
        let opts = temp_env::with_var(
            "APOLLO_ROVER_DEV_COMPOSITION_VERSION",
            Some("2.7.0"),
            || DevOpts::try_parse_from(["dev", "--composition-version", "2.9.0"]),
        )
        .unwrap();
        assert_that!(opts.supergraph_opts.composition_version)
            .is_equal_to(Some("2.9.0".to_string()));
    }

    #[test]
    fn composition_version_env_var_applies_alone() {
        let opts = temp_env::with_var(
            "APOLLO_ROVER_DEV_COMPOSITION_VERSION",
            Some("2.7.0"),
            || DevOpts::try_parse_from(["dev"]),
        )
        .unwrap();
        assert_that!(opts.supergraph_opts.composition_version)
            .is_equal_to(Some("2.7.0".to_string()));
    }
}
