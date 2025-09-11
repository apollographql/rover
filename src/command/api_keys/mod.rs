mod create;
mod delete;
mod list;

use std::fmt::{Display, Formatter};

use clap::{Parser, ValueEnum};
use rover_client::operations::api_keys::GraphOsKeyType;
use serde::Serialize;

use crate::command::api_keys::create::CreateKey;
use crate::command::api_keys::delete::DeleteKey;
use crate::command::api_keys::list::ListKeys;
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
    CreateKey(CreateKey),
    #[clap(name = "delete", about = "Delete an existing API key")]
    DeleteKey(DeleteKey),
    #[clap(name = "list", about = "List all API keys for an organization")]
    ListKeys(ListKeys),
}

impl ApiKeys {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::CreateKey(command) => command.run(client_config).await,
            Command::DeleteKey(command) => command.run(client_config).await,
            Command::ListKeys(command) => command.run(client_config).await,
        }
    }
}

// We define a new enum here so that we can keep the implementation details of the actual graph
// enum contained within this crate rather than leaking it out. Further it allows us to selectively
// add support for more key types as they are required, rather than them changing as the schema
// does.
#[derive(Debug, Clone, Serialize, ValueEnum, Copy)]
pub enum ApiKeyType {
    OPERATOR,
}

impl ApiKeyType {
    fn into_query_enum(self) -> GraphOsKeyType {
        match self {
            Self::OPERATOR => GraphOsKeyType::OPERATOR,
        }
    }
}

impl Display for ApiKeyType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiKeyType::OPERATOR => write!(f, "Operator"),
        }
    }
}
