mod create;
mod delete;
mod list;
mod rename;

use std::fmt::{Display, Formatter};

use clap::{Parser, ValueEnum};
use rover_client::operations::api_keys::GraphOsKeyType;
use serde::Serialize;

use crate::command::api_keys::create::Create;
use crate::command::api_keys::delete::Delete;
use crate::command::api_keys::list::List;
use crate::command::api_keys::rename::Rename;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct ApiKeys {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Serialize, Parser)]
pub enum Command {
    #[clap(name = "create", about = "Create a new API key")]
    Create(Create),
    #[clap(name = "delete", about = "Delete an existing API key")]
    Delete(Delete),
    #[clap(name = "list", about = "List all API keys for an organization")]
    List(List),
    #[clap(name = "rename", about = "Rename an existing API key")]
    Rename(Rename),
}

impl ApiKeys {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Create(command) => command.run(client_config).await,
            Command::Delete(command) => command.run(client_config).await,
            Command::List(command) => command.run(client_config).await,
            Command::Rename(command) => command.run(client_config).await,
        }
    }
}

// We define a new enum here so that we can keep the implementation details of the actual graph
// enum contained within this crate rather than leaking it out. Further it allows us to selectively
// add support for more key types as they are required, rather than them changing as the schema
// does.
#[derive(Debug, Clone, Serialize, ValueEnum, Copy)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiKeyType {
    Operator,
}

impl ApiKeyType {
    fn into_query_enum(self) -> GraphOsKeyType {
        match self {
            Self::Operator => GraphOsKeyType::OPERATOR,
        }
    }
}

impl Display for ApiKeyType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiKeyType::Operator => write!(f, "Operator"),
        }
    }
}

#[derive(Debug, Parser, Serialize)]
pub struct OrganizationOpt {
    #[clap(help = "The ID of the Organization")]
    organization_id: String,
}

#[derive(Debug, Parser, Serialize)]
pub struct IdOpt {
    #[clap(help = "The ID of the API key")]
    id: String,
}
