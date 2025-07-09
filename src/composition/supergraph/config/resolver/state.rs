use std::collections::BTreeMap;

use crate::composition::supergraph::config::federation::FederationVersionResolverFromSubgraphs;
use apollo_federation_types::config::SubgraphConfig;
use camino::Utf8PathBuf;
use rover_client::shared::GraphRef;

/// In this stage, we prompt the user to provide a subgraph if they have not provided any already
pub struct DefineDefaultSubgraph {
    pub origin_path: Option<Utf8PathBuf>,
    pub federation_version_resolver: FederationVersionResolverFromSubgraphs,
    pub subgraphs: BTreeMap<String, SubgraphConfig>,
    pub graph_ref: Option<GraphRef>,
}

/// In this stage, we attempt to resolve subgraphs lazily: making sure file paths are correct
/// and exist) or fully: rendering the subgraph source down to an SDL
pub struct ResolveSubgraphs {
    pub origin_path: Option<Utf8PathBuf>,
    pub federation_version_resolver: FederationVersionResolverFromSubgraphs,
    pub subgraphs: BTreeMap<String, SubgraphConfig>,
    pub graph_ref: Option<GraphRef>,
}
