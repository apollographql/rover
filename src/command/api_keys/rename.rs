use clap::Parser;
use rover_client::operations::api_keys::get::{GetKeyInput, run as run_get};
use rover_client::operations::api_keys::rename::{RenameKeyInput, run as run_rename};
use serde::Serialize;

use crate::command::api_keys::{IdOpt, OrganizationOpt};
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct Rename {
    #[clap(flatten)]
    profile: ProfileOpt,
    #[clap(flatten)]
    organization_opt: OrganizationOpt,
    #[clap(flatten)]
    id_opt: IdOpt,
    #[clap(help = "The new name of the key once it has been renamed")]
    new_name: String,
}

impl Rename {
    pub(crate) async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        let old_key_resp = run_get(
            GetKeyInput {
                organization_id: self.organization_opt.organization_id.clone(),
                key_id: self.id_opt.id.clone(),
            },
            &client,
        )
        .await?;

        let rename_resp = run_rename(
            RenameKeyInput {
                organization_id: self.organization_opt.organization_id.clone(),
                key_id: self.id_opt.id.clone(),
                new_name: self.new_name.clone(),
            },
            &client,
        )
        .await?;
        Ok(RoverOutput::RenameKeyResponse {
            id: rename_resp.key_id,
            old_name: old_key_resp.key.name,
            new_name: rename_resp.name,
        })
    }
}
