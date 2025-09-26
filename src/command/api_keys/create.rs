use clap::Parser;
use rover_client::operations::api_keys::create::{CreateKeyInput, run};
use serde::Serialize;

use crate::command::api_keys::ApiKeyType;
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct Create {
    #[clap(flatten)]
    profile: ProfileOpt,
    organization_id: String,
    name: String,
    #[clap(name = "type", value_enum)]
    key_type: ApiKeyType,
}

impl Create {
    pub(crate) async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        let resp = run(
            CreateKeyInput {
                organization_id: self.organization_id.clone(),
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
