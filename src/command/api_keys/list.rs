use clap::Parser;
use rover_client::operations::api_keys::list_keys::{ListKeysInput, run};
use serde::Serialize;

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct ListKeys {
    #[clap(flatten)]
    profile: ProfileOpt,
    organization_id: String,
    id: String,
}

impl ListKeys {
    pub(crate) async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let resp = run(
            ListKeysInput {
                organization_id: self.organization_id.clone(),
            },
            &client,
        )
        .await?;
        Ok(RoverOutput::ListKeysResponse { keys: resp.keys })
    }
}
