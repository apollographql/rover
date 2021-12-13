use crate::utils::client::StudioClientConfig;
use crate::{command::RoverOutput, Result};

use rover_client::operations::subgraph::list::{self, SubgraphListInput};
use rover_client::shared::GraphRef;

use ansi_term::Colour::{Cyan, Yellow};
use apollo_supergraph_config::{SchemaSource, SubgraphConfig, SupergraphConfig};
use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Init {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to fetch from.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Init {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        let graph_ref = self.graph.to_string();
        eprintln!(
            "Generating supergraph configuration from {} using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Yellow.normal().paint(&self.profile_name)
        );

        let subgraph_list_response = list::run(
            SubgraphListInput {
                graph_ref: self.graph.clone(),
            },
            &client,
        )?;

        let mut subgraphs: Vec<(String, SubgraphConfig)> = Vec::new();

        for subgraph in subgraph_list_response.subgraphs {
            subgraphs.push((
                subgraph.name.clone(),
                SubgraphConfig {
                    routing_url: subgraph.url,
                    schema: SchemaSource::Subgraph {
                        graphref: graph_ref.clone(),
                        subgraph: subgraph.name.clone(),
                    },
                },
            ));
        }

        let supergraph_config = SupergraphConfig::new(subgraphs.as_slice());

        Ok(RoverOutput::SupergraphConfig(serde_yaml::to_string(
            &supergraph_config,
        )?))
    }
}
