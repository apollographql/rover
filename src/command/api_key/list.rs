use clap::Parser;
use rover_client::operations::api_key::list::{ListKeysInput, run};
use serde::Serialize;

use crate::command::api_key::OrganizationOpt;
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct List {
    #[clap(flatten)]
    profile: ProfileOpt,
    #[clap(flatten)]
    organization_opt: OrganizationOpt,
}

impl List {
    pub(crate) async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let resp = run(
            ListKeysInput {
                organization_id: self.organization_opt.organization_id.clone(),
            },
            &client,
        )
        .await?;
        Ok(RoverOutput::ListKeysResponse { keys: resp.keys })
    }
}
