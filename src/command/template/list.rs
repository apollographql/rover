use clap::Parser;
use serde::Serialize;

use crate::options::TemplateOpt;
use crate::{RoverOutput, RoverResult};

use super::templates::list_templates;

#[derive(Clone, Debug, Parser, Serialize)]
pub struct List {
    #[clap(flatten)]
    options: TemplateOpt,
}

impl List {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        let templates = list_templates(self.options.language.clone())?;
        Ok(RoverOutput::TemplateList(templates))
    }
}
