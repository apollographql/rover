use clap::Parser;
use rover_std::Style;
use serde::Serialize;

use crate::options::{GraphRefOpt, ProfileOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

use rover_client::operations::persisted_queries::publish::{self, PersistedQueriesPublishInput};

#[derive(Debug, Serialize, Parser)]
pub struct Publish {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Publish {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let graph_ref = self.graph.graph_ref.to_string();

        eprintln!(
            "Persisting queries for {} using credentials from the {} profile.",
            Style::Link.paint(&graph_ref),
            Style::Command.paint(&self.profile.profile_name)
        );
        let result = publish::run(
            PersistedQueriesPublishInput {
                graph_ref: self.graph.graph_ref.clone(),
            },
            &client,
        )?;
        Ok(RoverOutput::PersistedQueriesPublishResponse(result))
    }
}
