use buildstructor::buildstructor;
use reqwest::Url;
use saucer::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

/// Config for a single [subgraph](https://www.apollographql.com/docs/federation/subgraphs/)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphConfig {
    /// The routing URL for the subgraph.
    /// This will appear in supergraph SDL and
    /// instructs the graph router to send all requests
    /// for this subgraph to this URL.
    pub remote_endpoint: Option<Url>,

    /// The routing URL for the subgraph when run locally.
    /// This will appear in supergraph SDL
    /// and instructs the graph router to send requests
    /// for this subgraph to this URL.
    pub local_endpoint: Url,

    /// The location of the subgraph's SDL
    pub schema: SchemaSource,
}

#[buildstructor]
impl SubgraphConfig {
    #[builder(entry = "schema")]
    pub fn from_file<F>(file: F, local_endpoint: Url, remote_endpoint: Option<Url>) -> Self
    where
        F: AsRef<Utf8Path>,
    {
        let file = file.as_ref().to_path_buf();
        Self {
            schema: SchemaSource::File { file },
            local_endpoint,
            remote_endpoint,
        }
    }

    #[builder(entry = "introspect")]
    pub fn from_subgraph_introspect(subgraph_url: Url, local: Url, remote: Option<Url>) -> Self {
        Self {
            schema: SchemaSource::SubgraphIntrospection { subgraph_url },
            local_endpoint: local,
            remote_endpoint: remote,
        }
    }

    #[builder(entry = "studio")]
    pub fn from_studio(
        graphref: String,
        subgraph_name: Option<String>,
        local: Url,
        remote: Option<Url>,
    ) -> Self {
        Self {
            schema: SchemaSource::Studio {
                graphref,
                subgraph: subgraph_name,
            },
            local_endpoint: local,
            remote_endpoint: remote,
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
    File {
        file: Utf8PathBuf,
    },
    SubgraphIntrospection {
        subgraph_url: Url,
    },
    Studio {
        graphref: String,
        subgraph: Option<String>,
    },
}
