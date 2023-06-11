use clap::Parser;
use serde::Serialize;

use crate::options::MergeOpt;
use crate::{RoverOutput, RoverResult};

use super::tools::merge_schemas;

#[derive(Clone, Debug, Parser, Serialize)]
pub struct Merge {
    #[clap(flatten)]
    options: MergeOpt,
}

impl Merge {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        let schema = merge_schemas(self.options.schemas.clone())?;
        Ok(RoverOutput::ToolsSchemaMerge(schema))
    }
}
