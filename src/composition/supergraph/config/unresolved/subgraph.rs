use std::sync::Arc;

use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use camino::Utf8PathBuf;
use derive_getters::Getters;

use crate::composition::supergraph::config::error::ResolveSubgraphError;
use crate::composition::supergraph::config::lazy::LazilyResolvedSubgraph;

/// Represents a `SubgraphConfig` that needs to be resolved, either fully or lazily
#[derive(Clone, Debug, Getters)]
pub struct UnresolvedSubgraph {
    name: String,
    schema: SchemaSource,
    routing_url: Option<String>,
}

impl UnresolvedSubgraph {
    /// Constructs an [`UnresolvedSubgraph`] from a subgraph name and [`SubgraphConfig`]
    pub fn new(name: String, config: SubgraphConfig) -> UnresolvedSubgraph {
        UnresolvedSubgraph {
            name,
            schema: config.schema,
            routing_url: config.routing_url,
        }
    }

    /// Produces a canonical filepath as the path relates to the supplied root path
    pub fn resolve_file_path(
        &self,
        root: &Utf8PathBuf,
        path: &Utf8PathBuf,
    ) -> Result<Utf8PathBuf, ResolveSubgraphError> {
        let joined_path = root.join(path);
        let canonical_filename = joined_path.canonicalize_utf8();
        match canonical_filename {
            Ok(canonical_filename) => Ok(canonical_filename),
            Err(err) => Err(ResolveSubgraphError::FileNotFound {
                subgraph_name: self.name.to_string(),
                supergraph_config_path: root.clone(),
                path: path.clone(),
                joined_path,
                source: Arc::new(err),
            }),
        }
    }
}

impl From<UnresolvedSubgraph> for SubgraphConfig {
    fn from(value: UnresolvedSubgraph) -> Self {
        SubgraphConfig {
            routing_url: value.routing_url,
            schema: value.schema,
        }
    }
}

impl From<LazilyResolvedSubgraph> for UnresolvedSubgraph {
    fn from(value: LazilyResolvedSubgraph) -> Self {
        UnresolvedSubgraph {
            name: value.name().to_string(),
            routing_url: value.routing_url().clone(),
            schema: value.schema().clone(),
        }
    }
}
