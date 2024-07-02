use apollo_federation_types::config::{FederationVersion, SubgraphConfig, SupergraphConfig};
use clap::{Parser, Subcommand};
use rover_client::operations::subgraph::{self, list::SubgraphListInput};
use rover_std::Style;
use serde::Serialize;

use crate::{
    options::{GraphRefOpt, ProfileOpt},
    utils::client::StudioClientConfig,
    RoverOutput, RoverResult,
};

#[derive(Debug, Serialize, Subcommand)]
pub enum Config {
    Fetch(ConfigFetch),
}

impl Config {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match self {
            Config::Fetch(command) => command.run(client_config),
        }
    }
}

#[derive(Debug, Serialize, Parser)]
pub struct ConfigFetch {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    #[clap(long)]
    federation_version: Option<FederationVersion>,
}

impl ConfigFetch {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.to_string();
        eprintln!(
            "Fetching supergraph config from {} using credentials from the {} profile.",
            Style::Link.paint(graph_ref),
            Style::Command.paint(&self.profile.profile_name)
        );

        let graph_ref = &self.graph.graph_ref;

        let subgraph_list_response = subgraph::list::run(
            SubgraphListInput {
                graph_ref: graph_ref.clone(),
            },
            &client,
        )?;

        let subgraph_configs = subgraph_list_response
            .subgraphs
            .iter()
            .map(|subgraph| {
                let subgraph_name = &subgraph.name;
                let config = SubgraphConfig {
                    routing_url: subgraph.url.clone(),
                    schema: apollo_federation_types::config::SchemaSource::Subgraph {
                        graphref: graph_ref.to_string(),
                        subgraph: subgraph_name.to_string(),
                    },
                };
                (subgraph_name.to_string(), config)
            })
            .collect();

        let mut supergraph_config = SupergraphConfig::new(subgraph_configs, None);
        let federation_version = self
            .federation_version
            .clone()
            .unwrap_or(FederationVersion::LatestFedTwo);
        supergraph_config.set_federation_version(federation_version);

        Ok(RoverOutput::SupergraphConfigFetchResponse(
            supergraph_config,
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use apollo_federation_types::config::{
        FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
    };
    use camino::Utf8PathBuf;
    use httpmock::{Method, MockServer};
    use rover_client::shared::GraphRef;
    use serde_json::json;
    use speculoos::prelude::*;

    use crate::{
        options::{GraphRefOpt, ProfileOpt},
        utils::client::{ClientBuilder, StudioClientConfig},
        RoverOutput,
    };

    use super::ConfigFetch;

    #[test]
    fn test_supergraph_config_fetch() {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(Method::POST);
            let response = json!({
                "data": {
                    "frontendUrlRoot": "https://studio.apollographql.com/",
                    "graph": {
                        "variant": {
                            "subgraphs": [
                                {
                                    "name": "actors",
                                    "url": "https://example.com/graphql",
                                    "updatedAt": "2024-05-27T02:20:21.261Z"
                                }
                            ]
                        }
                    }
                }
            });
            then.status(200)
                .header("content-type", "application/json")
                .json_body(response);
        });
        let studio_client_config = StudioClientConfig::new(
            Some(server.base_url()),
            houston::Config::new(None::<&Utf8PathBuf>, None).unwrap(),
            false,
            ClientBuilder::default(),
        );
        let command = ConfigFetch {
            graph: GraphRefOpt {
                graph_ref: GraphRef::new("test".to_string(), Some("current".to_string())).unwrap(),
            },
            profile: ProfileOpt {
                profile_name: "default".to_string(),
            },
            federation_version: None,
        };

        let result = command.run(studio_client_config);
        mock.assert_hits(1);

        let expected_supergraph_config = SupergraphConfig::new(
            BTreeMap::from([(
                "actors".to_string(),
                SubgraphConfig {
                    routing_url: Some("https://example.com/graphql".to_string()),
                    schema: SchemaSource::Subgraph {
                        graphref: "test@current".to_string(),
                        subgraph: "actors".to_string(),
                    },
                },
            )]),
            Some(FederationVersion::LatestFedTwo),
        );

        let expected_output =
            RoverOutput::SupergraphConfigFetchResponse(expected_supergraph_config);

        assert_that!(result).is_ok().is_equal_to(expected_output);
    }
}
