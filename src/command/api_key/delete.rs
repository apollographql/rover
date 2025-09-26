use clap::Parser;
use rover_client::operations::api_key::delete::{DeleteKeyInput, run};
use serde::Serialize;

use crate::command::api_key::{IdOpt, OrganizationOpt};
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct Delete {
    #[clap(flatten)]
    profile: ProfileOpt,
    #[clap(flatten)]
    organisation_opt: OrganizationOpt,
    #[clap(flatten)]
    id_opt: IdOpt,
}

impl Delete {
    pub(crate) async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let resp = run(
            DeleteKeyInput {
                organization_id: self.organisation_opt.organization_id.clone(),
                key_id: self.id_opt.id.clone(),
            },
            &client,
        )
        .await?;
        Ok(RoverOutput::DeleteKeyResponse { id: resp.key_id })
    }
}
