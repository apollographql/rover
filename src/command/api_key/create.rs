use clap::Parser;
use rover_client::operations::api_key::create::{CreateKeyInput, run};
use serde::Serialize;

use crate::command::api_key::{ApiKeyType, OrganizationOpt};
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct Create {
    #[clap(flatten)]
    profile: ProfileOpt,
    #[clap(flatten)]
    organization_opt: OrganizationOpt,
    #[clap(name = "TYPE", value_enum, help = "The type of the API key")]
    key_type: ApiKeyType,
    #[clap(help = "The name of the key to be created")]
    name: String,
}

impl Create {
    pub(crate) async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let resp = run(
            CreateKeyInput {
                organization_id: self.organization_opt.organization_id.clone(),
                name: self.name.clone(),
                key_type: self.key_type.into_query_enum(),
            },
            &client,
        )
        .await?;
        Ok(RoverOutput::CreateKeyResponse {
            api_key: resp.token,
            key_type: self.key_type.to_string(),
            id: resp.key_id,
            name: resp.key_name,
        })
    }
}
