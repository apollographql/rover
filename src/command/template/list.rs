use clap::Parser;
use serde::Serialize;

use super::templates::list_templates;
use crate::{RoverOutput, RoverResult, options::TemplateOpt};

#[derive(Clone, Debug, Parser, Serialize)]
pub struct List {
    #[clap(flatten)]
    options: TemplateOpt,
}

impl List {
    pub async fn run(&self, templates_api: Option<&str>) -> RoverResult<RoverOutput> {
        let templates = list_templates(self.options.language.clone(), templates_api).await?;
        Ok(RoverOutput::TemplateList(templates))
    }
}
