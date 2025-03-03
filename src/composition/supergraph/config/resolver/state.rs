use std::collections::BTreeMap;

use apollo_federation_types::config::SubgraphConfig;
use camino::Utf8PathBuf;

use crate::composition::supergraph::config::federation::{
    FederationVersionResolverFromSubgraphs, FederationVersionResolverFromSupergraphConfig,
};

/// In this stage, we await the caller to optionally load subgraphs from the Studio API using
/// the contents of the `--graph-ref` flag
pub struct LoadRemoteSubgraphs {
    pub federation_version_resolver: FederationVersionResolverFromSupergraphConfig,
}

/// In this stage, we await the caller to optionally load subgraphs and a specified federation
/// version from a local supergraph config file
pub struct LoadSupergraphConfig {
    pub federation_version_resolver: FederationVersionResolverFromSupergraphConfig,
    pub subgraphs: BTreeMap<String, SubgraphConfig>,
}

/// In this stage, we prompt the user to provide a subgraph if they have not provided any already
pub struct DefineDefaultSubgraph {
    pub origin_path: Option<Utf8PathBuf>,
    pub federation_version_resolver: FederationVersionResolverFromSubgraphs,
    pub subgraphs: BTreeMap<String, SubgraphConfig>,
}

/// In this stage, we attempt to resolve subgraphs lazily: making sure file paths are correct
/// and exist) or fully: rendering the subgraph source down to an SDL
pub struct ResolveSubgraphs {
    pub origin_path: Option<Utf8PathBuf>,
    pub federation_version_resolver: FederationVersionResolverFromSubgraphs,
    pub subgraphs: BTreeMap<String, SubgraphConfig>,
}
