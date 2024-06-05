use crate::command::supergraph::compose::CompositionOutput;

use anyhow::Result;
use apollo_federation_types::javascript::SubgraphDefinition;
use reqwest::Url;

pub type SubgraphName = String;
pub type SubgraphUrl = Url;
pub type SubgraphSdl = String;
pub type SubgraphKey = (SubgraphName, SubgraphUrl);
pub type SubgraphKeys = Vec<SubgraphKey>;
pub type SubgraphEntry = (SubgraphKey, SubgraphSdl);
pub type CompositionResult = std::result::Result<Option<CompositionOutput>, String>;

pub(crate) fn sdl_from_definition(subgraph_definition: &SubgraphDefinition) -> SubgraphSdl {
    subgraph_definition.sdl.to_string()
}

pub(crate) fn name_from_definition(subgraph_definition: &SubgraphDefinition) -> SubgraphName {
    subgraph_definition.name.to_string()
}

pub(crate) fn url_from_definition(subgraph_definition: &SubgraphDefinition) -> Result<SubgraphUrl> {
    Ok(subgraph_definition.url.parse()?)
}

pub(crate) fn key_from_definition(subgraph_definition: &SubgraphDefinition) -> Result<SubgraphKey> {
    Ok((
        name_from_definition(subgraph_definition),
        url_from_definition(subgraph_definition)?,
    ))
}

pub(crate) fn entry_from_definition(
    subgraph_definition: &SubgraphDefinition,
) -> Result<SubgraphEntry> {
    Ok((
        key_from_definition(subgraph_definition)?,
        sdl_from_definition(subgraph_definition),
    ))
}
