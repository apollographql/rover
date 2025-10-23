use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use buildstructor::Builder;
use camino::Utf8PathBuf;
use derive_getters::Getters;

use crate::composition::supergraph::config::{
    error::ResolveSubgraphError, unresolved::UnresolvedSubgraph,
};

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
        name: String,
        unresolved_subgraph: SubgraphConfig,
    ) -> Result<LazilyResolvedSubgraph, ResolveSubgraphError> {
        match unresolved_subgraph.schema {
            SchemaSource::File { file } => {
                let file = UnresolvedSubgraph::resolve_file_path(
                    &name,
                    supergraph_config_root,
                    &Utf8PathBuf::try_from(file)?,
                )?;
                Ok(LazilyResolvedSubgraph {
                    name,
                    routing_url: unresolved_subgraph.routing_url,
                    schema: SchemaSource::File {
                        file: file.into_std_path_buf(),
                    },
                })
            }
            schema => Ok(LazilyResolvedSubgraph {
                name,
                routing_url: unresolved_subgraph.routing_url,
                schema,
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
