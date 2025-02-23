use apollo_federation_types::config::SupergraphConfig;
use clap::Parser;
use schemars::schema_for;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Schema {}

impl Schema {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        let schema = schema_for!(SupergraphConfig);
        Ok(RoverOutput::JsonSchema(
            serde_json::to_string_pretty(&schema).unwrap(),
        ))
    }
}
