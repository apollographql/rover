use crate::federation::supergraph_config::ResolvedSupergraphConfig;
use camino::Utf8PathBuf;

/// Takes the configuration for composing a supergraph and composes it. Also can watch that file and
/// all subgraphs for changes, recomposing and emitting events when they occur.
// TODO: nice constructor & channels instead of pub fields
#[derive(Debug)]
pub(crate) struct Composer {
    #[allow(dead_code)]
    pub(crate) supergraph_yaml_path: Option<Utf8PathBuf>,
    pub(crate) supergraph_config: ResolvedSupergraphConfig,
}

impl Composer {
    /// Create a new composer using `initial_config` for the first composition, and then watching
    /// `supergraph_yaml_path` for changes.
    pub(crate) fn new(
        initial_config: ResolvedSupergraphConfig,
        supergraph_yaml_path: Option<Utf8PathBuf>,
    ) -> Self {
        Self {
            supergraph_yaml_path,
            supergraph_config: initial_config,
        }
    }
}
