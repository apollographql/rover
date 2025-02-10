use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use buildstructor::Builder;
use camino::Utf8PathBuf;
use derive_getters::Getters;

use crate::composition::supergraph::config::error::ResolveSubgraphError;
use crate::composition::supergraph::config::unresolved::UnresolvedSubgraph;

/// A subgraph config that has had its file paths validated and
/// confirmed to be relative to a supergraph config file
#[derive(Clone, Debug, Eq, PartialEq, Getters, Builder)]
pub struct LazilyResolvedSubgraph {
    name: String,
    routing_url: Option<String>,
    schema: SchemaSource,
}

impl LazilyResolvedSubgraph {
    /// Resolves a [`UnresolvedSubgraph`] to a [`LazilyResolvedSubgraph`] by validating
    /// any filepaths and confirming that they are relative to a supergraph config schema
    pub fn resolve(
        supergraph_config_root: &Utf8PathBuf,
        unresolved_subgraph: UnresolvedSubgraph,
    ) -> Result<LazilyResolvedSubgraph, ResolveSubgraphError> {
        match unresolved_subgraph.schema() {
            SchemaSource::File { file } => {
                let file = unresolved_subgraph.resolve_file_path(
                    &supergraph_config_root.clone(),
                    &Utf8PathBuf::try_from(file.clone())?,
                )?;
                Ok(LazilyResolvedSubgraph {
                    name: unresolved_subgraph.name().to_string(),
                    routing_url: unresolved_subgraph.routing_url().clone(),
                    schema: SchemaSource::File {
                        file: file.into_std_path_buf(),
                    },
                })
            }
            _ => Ok(LazilyResolvedSubgraph {
                name: unresolved_subgraph.name().to_string(),
                routing_url: unresolved_subgraph.routing_url().clone(),
                schema: unresolved_subgraph.schema().clone(),
            }),
        }
    }
}

impl From<LazilyResolvedSubgraph> for SubgraphConfig {
    fn from(value: LazilyResolvedSubgraph) -> Self {
        SubgraphConfig {
            routing_url: value.routing_url,
            schema: value.schema,
        }
    }
}
