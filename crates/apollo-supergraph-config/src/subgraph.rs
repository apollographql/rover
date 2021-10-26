use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use url::Url;

/// Config for a single [subgraph](https://www.apollographql.com/docs/federation/subgraphs/)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphConfig {
    /// The routing URL for the subgraph.
    /// This will appear in supergraph SDL and
    /// instructs the graph router to send all requests
    /// for this subgraph to this URL.
    pub routing_url: Option<String>,

    /// The location of the subgraph's SDL
    pub schema: SchemaSource,
}

impl SubgraphConfig {
    /// Returns SDL from the configuration file if it exists.
    /// Returns None if the configuration does not include raw SDL.
    pub fn get_sdl(&self) -> Option<String> {
        if let SchemaSource::Sdl { sdl } = &self.schema {
            Some(sdl.to_owned())
        } else {
            None
        }
    }
}

/// Options for getting SDL:
/// the graph registry, a file, or an introspection URL.
///
/// NOTE: Introspection strips all comments and directives
/// from the SDL.
#[derive(Debug, Clone, Serialize, Deserialize)]
// this is untagged, meaning its fields will be flattened into the parent
// struct when de/serialized. There is no top level `schema_source`
// in the configuration.
#[serde(untagged)]
pub enum SchemaSource {
    File { file: Utf8PathBuf },
    SubgraphIntrospection { subgraph_url: Url },
    Subgraph { graphref: String, subgraph: String },
    Sdl { sdl: String },
}
