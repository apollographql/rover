use std::collections::BTreeMap;

use apollo_federation_types::config::{SchemaSource, SubgraphConfig, SupergraphConfig};
use thiserror::Error;

/// Error that occurs when a subgraph schema source is invalid
#[derive(Error, Debug)]
#[error("Invalid schema source: {:?}", .schema_source)]
pub struct InvalidSchemaSource {
    schema_source: SchemaSource,
}

/// Object that contains the completed set of subgraphs resolved to their SDLs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullyResolvedSubgraphs {
    subgraphs: BTreeMap<String, String>,
}

impl FullyResolvedSubgraphs {
    #[cfg(test)]
    pub fn new(subgraphs: BTreeMap<String, String>) -> FullyResolvedSubgraphs {
        FullyResolvedSubgraphs { subgraphs }
    }

    /// Used to upsert a fully resolved subgraph into this object's definitions
    pub fn upsert_subgraph(&mut self, name: String, schema: String) {
        self.subgraphs.insert(name, schema);
    }

    /// Removes a subgraph from this object's definitions
    pub fn remove_subgraph(&mut self, name: &str) {
        self.subgraphs.remove(name);
    }
}

impl TryFrom<SupergraphConfig> for FullyResolvedSubgraphs {
    type Error = Vec<InvalidSchemaSource>;
    fn try_from(value: SupergraphConfig) -> Result<Self, Self::Error> {
        let mut errors = Vec::new();
        let mut subgraph_sdls = BTreeMap::new();
        for (name, subgraph_config) in value.into_iter() {
            if let SchemaSource::Sdl { sdl } = subgraph_config.schema {
                subgraph_sdls.insert(name, sdl);
            } else {
                errors.push(InvalidSchemaSource {
                    schema_source: subgraph_config.schema,
                });
            }
        }
        if errors.is_empty() {
            Ok(FullyResolvedSubgraphs {
                subgraphs: subgraph_sdls,
            })
        } else {
            Err(errors)
        }
    }
}

impl From<FullyResolvedSubgraphs> for SupergraphConfig {
    fn from(value: FullyResolvedSubgraphs) -> Self {
        let subgraphs = BTreeMap::from_iter(value.subgraphs.into_iter().map(|(name, sdl)| {
            (
                name,
                SubgraphConfig {
                    routing_url: None,
                    schema: SchemaSource::Sdl { sdl },
                },
            )
        }));
        SupergraphConfig::new(subgraphs, None)
    }
}
