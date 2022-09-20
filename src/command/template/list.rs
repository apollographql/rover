use saucer::{clap, Parser};
use serde::Serialize;

use crate::command::template::templates::GithubTemplates;
use crate::options::TemplateOpt;
use crate::{command::RoverOutput, Result};

#[derive(Clone, Debug, Parser, Serialize)]
pub struct List {
    #[clap(flatten)]
    options: TemplateOpt,
}

impl List {
    pub fn run(&self) -> Result<RoverOutput> {
        let mut templates = GithubTemplates::new();
        if let Some(project_language) = self.options.language {
            templates.filter_language(project_language);
        }
        templates.error_on_empty()?;
        Ok(RoverOutput::TemplateList(templates.values()))
    }
}
