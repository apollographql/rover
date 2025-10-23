use std::collections::BTreeMap;

use apollo_federation_types::config::{FederationVersion, SubgraphConfig};
use serde::{Deserialize, Serialize};

/// The YAML that a user will write to configure a supergraph.
#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SupergraphConfigYaml {
    // Store config in a BTreeMap, as HashMap is non-deterministic.
    pub(crate) subgraphs: BTreeMap<String, SubgraphConfig>,

    // The version requirement for the supergraph binary.
    pub(crate) federation_version: Option<FederationVersion>,
}
