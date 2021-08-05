use ansi_term::Colour::Cyan;
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::subgraph::list;

use crate::command::RoverStdout;
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::{parse_graph_ref, GraphRef};
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct List {
    /// <NAME>@<VARIANT> of graph in Apollo Studio to list subgraphs from.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF", parse(try_from_str = parse_graph_ref))]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl List {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;

        eprintln!(
            "Listing subgraphs for {} using credentials from the {} profile.",
            Cyan.normal().paint(self.graph.to_string()),
            Cyan.normal().paint(&self.profile_name)
        );

        let list_details = list::run(
            list::list_subgraphs_query::Variables {
                graph_id: self.graph.name.clone(),
                variant: self.graph.variant.clone(),
            },
            &client,
        )?;

        Ok(RoverStdout::SubgraphList(list_details))
    }
}
