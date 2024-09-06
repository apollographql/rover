use std::str::FromStr;

use apollo_federation_types::config::{FederationVersion, SchemaSource, SubgraphConfig};
use buildstructor::Builder;
use derive_getters::Getters;

pub(crate) use subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariant;
pub(crate) use subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariant;

use crate::shared::GraphRef;

use super::runner::subgraph_fetch_all_query;

use subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantLatestLaunchBuildInput::CompositionBuildInput
  as OuterCompositionBuildInput;
use subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariantLatestLaunchBuildInput::CompositionBuildInput
  as InnerCompositionBuildInput;

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

#[derive(Debug, PartialEq)]
pub struct SubgraphFetchAllResponse {
    pub subgraphs: Vec<Subgraph>,
    pub federation_version: Option<FederationVersion>,
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

impl From<subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSubgraphs>
    for Subgraph
{
    fn from(
        value: subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSubgraphs,
    ) -> Self {
        Subgraph::builder()
            .name(value.name)
            .and_url(value.url)
            .sdl(value.active_partial_schema.sdl)
            .build()
    }
}

impl
    From<subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariantSubgraphs>
    for Subgraph
{
    fn from(
        value: subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariantSubgraphs,
    ) -> Self {
        Subgraph::builder()
            .name(value.name)
            .and_url(value.url)
            .sdl(value.active_partial_schema.sdl)
            .build()
    }
}

impl From<subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantLatestLaunch>
    for Option<FederationVersion>
{
    fn from(
        value: subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantLatestLaunch,
    ) -> Self {
        if let OuterCompositionBuildInput(composition_build_input) = value.build_input {
            composition_build_input.version.as_ref().and_then(|v| FederationVersion::from_str(&("=".to_owned() + v)).ok())
        } else {
            None
        }
    }
}

impl From<subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariantLatestLaunch>
    for Option<FederationVersion>
{
    fn from(
        value: subgraph_fetch_all_query::SubgraphFetchAllQueryVariantOnGraphVariantSourceVariantLatestLaunch,
    ) -> Self {
        if let InnerCompositionBuildInput(composition_build_input) = value.build_input {
            composition_build_input.version.as_ref().and_then(|v| FederationVersion::from_str(&("=".to_owned() + v)).ok())
        } else {
            None
        }
    }
}
