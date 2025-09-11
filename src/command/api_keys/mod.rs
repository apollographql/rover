use std::fmt::{Display, Formatter};

use clap::{Parser, ValueEnum};
use rover_client::operations::api_keys::create_key::create_key_mutation::GraphOsKeyType as QueryGraphOsKeyType;
use rover_client::operations::api_keys::create_key::{CreateKeyInput, run};
use serde::Serialize;

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct ApiKeys {
    #[clap(subcommand)]
    command: Command,
}

impl ApiKeys {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::CreateKey(command) => command.run(client_config).await,
        }
    }
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    /// Manage Cloud Router config.
    CreateKey(CreateKey),
}

// We define a new enum here so that we can keep the implementation details of the actual graph
// enum contained within this crate rather than leaking it out. Further it allows us to selectively
// add support for more key types as they are required, rather than them changing as the schema
// does.
#[derive(Debug, Clone, Serialize, ValueEnum, Copy)]
pub enum GraphOsKeyType {
    OPERATOR,
}

impl GraphOsKeyType {
    fn into_query_enum(self) -> QueryGraphOsKeyType {
        match self {
            Self::OPERATOR => QueryGraphOsKeyType::OPERATOR,
        }
    }
}

impl Display for GraphOsKeyType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphOsKeyType::OPERATOR => write!(f, "Operator"),
        }
    }
}

#[derive(Debug, Serialize, Parser)]
pub(crate) struct CreateKey {
    #[clap(flatten)]
    profile: ProfileOpt,
    organization_id: String,
    name: String,
    #[clap(name = "type", value_enum)]
    key_type: GraphOsKeyType,
}

impl CreateKey {
    async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
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
