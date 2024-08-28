use std::net::IpAddr;

use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;

use rover_client::shared::GraphRef;

use crate::options::{OptionalSubgraphOpts, PluginOpts};
use crate::utils::parsers::FileDescriptorType;

#[cfg(feature = "composition-js")]
mod compose;

#[cfg(feature = "composition-js")]
mod do_dev;

#[cfg(feature = "composition-js")]
mod introspect;

#[cfg(feature = "composition-js")]
mod protocol;

#[cfg(feature = "composition-js")]
mod router;

#[cfg(feature = "composition-js")]
mod schema;

#[cfg(feature = "composition-js")]
mod netstat;

#[cfg(not(feature = "composition-js"))]
mod no_dev;

#[cfg(feature = "composition-js")]
mod watcher;

#[derive(Debug, Serialize, Parser)]
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
}

#[derive(Debug, Parser, Serialize, Clone)]
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
}

lazy_static::lazy_static! {
    pub(crate) static ref OVERRIDE_DEV_ROUTER_VERSION: Option<String> =
      std::env::var("APOLLO_ROVER_DEV_ROUTER_VERSION").ok();

    // this number should be mapped to the federation version used by the router
    // https://www.apollographql.com/docs/router/federation-version-support/#support-table
    pub(crate) static ref OVERRIDE_DEV_COMPOSITION_VERSION: Option<String> =
        std::env::var("APOLLO_ROVER_DEV_COMPOSITION_VERSION").ok();
}
