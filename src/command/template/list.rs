use saucer::{clap, Parser};
use serde::Serialize;

use crate::{command::RoverOutput, Result};
use crate::options::TemplateOpt;

use super::templates::GithubTemplates;

#[derive(Clone, Debug, Parser, Serialize)]
pub struct List {
    #[clap(flatten)]
    options: TemplateOpt,
}

impl List {
    pub fn run(&self) -> Result<RoverOutput> {
        let mut templates = GithubTemplates::new();
        if let Some(project_language) = self.options.language {
            templates = templates.filter_language(project_language);
        }
        Ok(RoverOutput::TemplateList(templates.values()?))
    }
}
