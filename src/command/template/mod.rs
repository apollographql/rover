pub(crate) mod custom_scalars;
mod list;
pub(crate) mod queries;
mod templates;
mod r#use;

pub use list::List;
pub use r#use::Use;

use clap::Parser;
use serde::Serialize;

use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Clone, Parser, Serialize)]
pub struct Template {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clone, Debug, Parser, Serialize)]
enum Command {
    /// Use a template to generate code
    Use(Use),

    /// List available templates that can be used
    List(List),
}

impl Template {
    pub(crate) async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        match &self.command {
            Command::Use(use_template) => use_template.run(client_config).await,
            Command::List(list) => list.run().await,
        }
    }
}
