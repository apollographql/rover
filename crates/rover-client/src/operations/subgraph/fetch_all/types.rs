use apollo_federation_types::config::FederationVersion;
use buildstructor::Builder;
use derive_getters::Getters;

use crate::shared::GraphRef;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphFetchAllInput {
    pub graph_ref: GraphRef,
}

#[derive(Debug, PartialEq)]
pub struct SubgraphFetchAllResponse {
    pub subgraphs: Vec<Subgraph>,
    pub federation_version: Option<FederationVersion>,
}

#[derive(Clone, Builder, Debug, Eq, Getters, PartialEq)]
pub struct Subgraph {
    pub name: String,
    pub url: Option<String>,
    pub sdl: String,
}
