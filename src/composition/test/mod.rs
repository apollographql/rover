use std::collections::BTreeMap;

use apollo_federation_types::{config::FederationVersion, rover::BuildHint};
use rstest::fixture;
use serde_json::json;

use super::CompositionSuccess;

const SUPERGRAPH_SDL: &str = include_str!("./supergraph.graphql");
const HINT: &str = "[UNUSED_ENUM_TYPE]: Enum type \"ShippingClass\" is defined but unused. It will be included in the supergraph with all the values appearing in any subgraph (\"as if\" it was only used as an output type).";

#[fixture]
pub fn default_composition_json() -> serde_json::Value {
    json!({
        "Ok": {
            "supergraphSdl": SUPERGRAPH_SDL,
            "hints": [
                {
                    "message": HINT,
                    "code": "UNUSED_ENUM_TYPE",
                    "nodes": [],
                    "omittedNodesCount": 0
                }
            ]
        }
    })
}

#[fixture]
pub fn default_composition_success(
    #[default(FederationVersion::default())] federation_version: FederationVersion,
) -> CompositionSuccess {
    CompositionSuccess {
        supergraph_sdl: SUPERGRAPH_SDL.to_string(),
        hints: vec![BuildHint {
            message: HINT.to_string(),
            code: Some("UNUSED_ENUM_TYPE".to_string()),
            nodes: Some(Vec::default()),
            omitted_nodes_count: Some(0),
            other: BTreeMap::default(),
        }],
        federation_version,
    }
}
