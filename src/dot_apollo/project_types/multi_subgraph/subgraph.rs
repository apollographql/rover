use buildstructor::buildstructor;
use dialoguer::Input;
use reqwest::Url;
use saucer::{Fs, Result, Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;

use std::{collections::HashMap, str::FromStr};

use rover_client::blocking::GraphQLClient;
use rover_client::operations::graph::{self, fetch::GraphFetchInput};
use rover_client::operations::subgraph::introspect::{self, SubgraphIntrospectInput};
use rover_client::operations::subgraph::{self, fetch::SubgraphFetchInput};
use rover_client::shared::GraphRef;

/// Config for a single [subgraph](https://www.apollographql.com/docs/federation/subgraphs/)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubgraphConfig {
    /// The routing URL for the subgraph.
    /// This will appear in supergraph SDL and
    /// instructs the graph router to send all requests
    /// for this subgraph to this URL.
    pub remote_endpoint: Option<String>,

    /// The routing URL for the subgraph when run locally.
    /// This will appear in supergraph SDL
    /// and instructs the graph router to send requests
    /// for this subgraph to this URL.
    pub local_endpoint: String,

    /// The location of the subgraph's SDL
    pub schema: SchemaSource,
}

impl SubgraphConfig {
    pub fn url(&self, dev: bool) -> Result<String> {
        if dev {
            Ok(self.local_endpoint.to_string())
        } else {
            if let Some(remote_endpoint) = &self.remote_endpoint {
                Ok(remote_endpoint.to_string())
            } else {
                let remote_endpoint: String = Input::new()
                    .with_prompt("What endpoint is your subgraph deployed to?")
                    .interact_text()?;
                Ok(remote_endpoint)
            }
        }
    }

    pub fn sdl(
        &self,
        client_config: &StudioClientConfig,
        profile_opt: &ProfileOpt,
    ) -> Result<String> {
        self.schema.resolve(client_config, profile_opt)
    }
}

#[buildstructor]
impl SubgraphConfig {
    #[builder(entry = "schema")]
    pub fn from_file<F>(file: F, local_endpoint: String, remote_endpoint: Option<String>) -> Self
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
    pub fn from_subgraph_introspect(
        subgraph_url: String,
        local: String,
        remote: Option<String>,
    ) -> Self {
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
        local: String,
        remote: Option<String>,
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

    pub fn edit_remote_endpoint(&mut self, remote_endpoint: String) {
        self.remote_endpoint = Some(remote_endpoint);
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
        subgraph_url: String,
    },
    Studio {
        graphref: String,
        subgraph: Option<String>,
    },
}

impl SchemaSource {
    pub fn resolve(
        &self,
        client_config: &StudioClientConfig,
        profile_opt: &ProfileOpt,
    ) -> Result<String> {
        match &self {
            SchemaSource::File { file } => Fs::read_file(file, ""),
            SchemaSource::SubgraphIntrospection { subgraph_url } => {
                // given a federated introspection URL, use subgraph introspect to
                // obtain SDL and add it to subgraph_definition.
                let client =
                    GraphQLClient::new(subgraph_url.as_ref(), client_config.get_reqwest_client())?;

                let introspection_response = introspect::run(
                    SubgraphIntrospectInput {
                        headers: HashMap::new(),
                    },
                    &client,
                )?;
                Ok(introspection_response.result)
            }
            SchemaSource::Studio {
                graphref: graph_ref,
                subgraph,
            } => {
                // given a graph_ref and subgraph, run subgraph fetch to
                // obtain SDL and add it to subgraph_definition.
                let client = client_config.get_authenticated_client(&profile_opt)?;
                let graph_ref = GraphRef::from_str(graph_ref)?;
                if let Some(subgraph) = subgraph {
                    let result = subgraph::fetch::run(
                        SubgraphFetchInput {
                            graph_ref,
                            subgraph_name: subgraph.clone(),
                        },
                        &client,
                    )?;

                    Ok(result.sdl.contents)
                } else {
                    let result = graph::fetch::run(GraphFetchInput { graph_ref }, &client)?;
                    Ok(result.sdl.contents)
                }
            }
        }
    }
}
