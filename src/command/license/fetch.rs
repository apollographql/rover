use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};
use clap::Parser;
use rover_client::operations::license::fetch::LicenseFetchInput;
use rover_std::{Spinner, Style};
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct Fetch {
    /// The Graph ID to fetch the license for.
    #[serde(skip_serializing)]
    #[arg(long)]
    graph_id: String,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Fetch {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let spinner = Spinner::new(&format!(
            "Fetching license for {} using credentials from the {} profile.",
            Style::Link.paint(&self.graph_id),
            Style::Command.paint(&self.profile.profile_name)
        ));

        let jwt = rover_client::operations::license::fetch::run(
            LicenseFetchInput {
                graph_id: self.graph_id.to_string(),
            },
            &client,
        )
        .await?;

        spinner.stop();

        Ok(RoverOutput::LicenseResponse {
            graph_id: self.graph_id.to_string(),
            jwt,
        })
    }
}
