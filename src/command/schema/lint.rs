use clap::Parser;
use serde::Serialize;

use apollo_compiler::ApolloCompiler;

use crate::command::RoverOutput;
use crate::error::RoverError;
use crate::options::SchemaOpt;
use crate::{anyhow, Result};

#[derive(Debug, Serialize, Parser)]
pub struct Lint {
    #[clap(flatten)]
    #[serde(skip_serializing)]
    schema: SchemaOpt,
}

impl Lint {
    pub fn run(&self) -> Result<RoverOutput> {
        let proposed_schema = self
            .schema
            .read_file_descriptor("SDL", &mut std::io::stdin())?;
        let compiler_context = ApolloCompiler::new(&proposed_schema);
        eprintln!(
            "Validating that {} conforms to the GraphQL specification.",
            self.schema
        );
        let errors = compiler_context.validate();
        if !errors.is_empty() {
            errors.iter().for_each(|e| eprintln!("{}", e));
            let num_errors = errors.len();
            Err(RoverError::new(anyhow!(
                "The schema contained {} error{}.",
                num_errors,
                match num_errors {
                    1 => "",
                    _ => "s",
                }
            )))
        } else {
            eprintln!("✅ Success! The schema contained 0 errors.");
            Ok(RoverOutput::EmptySuccess)
        }
    }
}
