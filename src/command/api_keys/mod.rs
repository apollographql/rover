mod create;
mod delete;

use std::fmt::{Display, Formatter};

use clap::{Parser, ValueEnum};
use rover_client::operations::api_keys::create_key::create_key_mutation::GraphOsKeyType as QueryGraphOsKeyType;
use serde::Serialize;

use crate::command::api_keys::create::CreateKey;
use crate::command::api_keys::delete::DeleteKey;
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
}

impl ApiKeys {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::CreateKey(command) => command.run(client_config).await,
            Command::DeleteKey(command) => command.run(client_config).await,
        }
    }
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
