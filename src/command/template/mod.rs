pub(crate) mod custom_scalars;
mod list;
pub(crate) mod queries;
mod templates;
mod r#use;

use clap::Parser;
pub use list::List;
use rover_http::ReqwestService;
use serde::Serialize;
pub use r#use::Use;

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
    pub(crate) async fn run(&self) -> RoverResult<RoverOutput> {
        let request_service = ReqwestService::builder().build()?;

        match &self.command {
            Command::Use(use_template) => use_template.run(request_service).await,
            Command::List(list) => list.run().await,
        }
    }
}
