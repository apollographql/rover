use ansi_term::Colour::Cyan;
use serde::Serialize;
use structopt::StructOpt;

use rover_client::query::variant::list;

use crate::command::RoverStdout;
use crate::utils::client::StudioClientConfig;
use crate::Result;

#[derive(Debug, Serialize, StructOpt)]
pub struct List {
    /// A unique graph identifier
    #[structopt(name = "graph_id")]
    #[serde(skip_serializing)]
    graph: String,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl List {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        let client = client_config.get_client(&self.profile_name)?;

        eprintln!(
            "Listing variants for {} using credentials from the {} profile.",
            Cyan.normal().paint(self.graph.to_string()),
            Cyan.normal().paint(&self.profile_name)
        );

        let list_details = list::run(
            list::list_variants_query::Variables {
                graph_id: self.graph.clone(),
            },
            &client,
        )?;

        Ok(RoverStdout::VariantList(list_details))
    }
}
