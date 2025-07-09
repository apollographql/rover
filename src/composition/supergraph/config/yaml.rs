use apollo_federation_types::config::{FederationVersion, SubgraphConfig};
use rover_client::shared::GraphRef;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The YAML that a user will write to configure a supergraph.
#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SupergraphConfigYaml {
    #[schemars(skip)] // TODO: Set up the right graph@variant schema
    pub(crate) graph_ref: Option<GraphRef>,

    // Store config in a BTreeMap, as HashMap is non-deterministic.
    pub(crate) subgraphs: BTreeMap<String, SubgraphConfig>,

    // The version requirement for the supergraph binary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) federation_version: Option<FederationVersion>,
}
