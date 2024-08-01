use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use buildstructor::Builder;
use derive_getters::Getters;

use crate::shared::GraphRef;

use super::runner::subgraph_fetch_all_query;

pub(crate) type SubgraphFetchAllResponseData = subgraph_fetch_all_query::ResponseData;
pub(crate) type SubgraphFetchAllGraphVariant =
    subgraph_fetch_all_query::SubgraphFetchAllQueryVariant;
pub(crate) type QueryVariables = subgraph_fetch_all_query::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphFetchAllInput {
    pub graph_ref: GraphRef,
}

impl From<SubgraphFetchAllInput> for QueryVariables {
    fn from(input: SubgraphFetchAllInput) -> Self {
        Self {
            graph_ref: input.graph_ref.to_string(),
        }
    }
}

#[derive(Clone, Builder, Debug, Eq, Getters, PartialEq)]
pub struct Subgraph {
    name: String,
    url: Option<String>,
    sdl: String,
}

impl From<Subgraph> for SubgraphConfig {
    fn from(value: Subgraph) -> Self {
        Self {
            routing_url: value.url,
            schema: SchemaSource::Sdl { sdl: value.sdl },
        }
    }
}
