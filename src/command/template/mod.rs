pub(crate) mod custom_scalars;
mod list;
pub(crate) mod queries;
mod templates;
mod r#use;

use clap::Parser;
pub use list::List;
use serde::Serialize;
pub use r#use::Use;

use crate::{RoverOutput, RoverResult, cli::Rover, options::TemplatesApiOpt};

#[derive(Debug, Clone, Parser, Serialize)]
pub struct Template {
    #[clap(subcommand)]
    command: Command,

    #[clap(flatten)]
    templates_api: TemplatesApiOpt,
}

#[derive(Clone, Debug, Parser, Serialize)]
enum Command {
    /// Use a template to generate code
    Use(Use),

    /// List available templates that can be used
    List(List),
}

impl Template {
    pub(crate) async fn run(&self, rover: &Rover) -> RoverResult<RoverOutput> {
        let templates_api =
            rover.resolve_templates_api(self.templates_api.templates_api.clone())?;
        let templates_api = templates_api.as_deref();
        match &self.command {
            Command::Use(use_template) => use_template.run(templates_api).await,
            Command::List(list) => list.run(templates_api).await,
        }
    }
}
