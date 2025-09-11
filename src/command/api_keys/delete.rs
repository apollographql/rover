use clap::Parser;
use rover_client::operations::api_keys::delete_key::{DeleteKeyInput, run};
use serde::Serialize;

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct DeleteKey {
    #[clap(flatten)]
    profile: ProfileOpt,
    organization_id: String,
    id: String,
}

impl DeleteKey {
    pub(crate) async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let resp = run(
            DeleteKeyInput {
                organization_id: self.organization_id.clone(),
                key_id: self.id.clone(),
            },
            &client,
        )
        .await?;
        Ok(RoverOutput::DeleteKeyResponse { id: resp.key_id })
    }
}
