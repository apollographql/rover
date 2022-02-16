use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;

use crate::command::supergraph::Template;
use crate::command::RoverOutput;
use crate::error::RoverError;

#[derive(Debug, Serialize, StructOpt)]
pub struct Generate {
    #[structopt(long = "template", possible_values = &Template::possible_templates())]
    template: Template,

    #[structopt(long = "directory", default_value = "./supergraph")]
    directory: Utf8PathBuf,
}

impl Generate {
    pub fn run(&self) -> Result<RoverOutput, RoverError> {
        let repository_url = self.template.get_repository_url().to_string();
        self.template.clone_repo(&self.directory)?;
        Ok(RoverOutput::SupergraphGenerate {
            repository_url,
            directory: self.directory.clone(),
        })
    }
}
