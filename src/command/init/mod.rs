use clap::Parser;
use serde::Serialize;
use crate::options::ProjectUseCaseOpt;
use crate::{RoverResult, RoverOutput};

#[derive(Debug, Serialize, Parser)]
pub struct Init {
    #[clap(flatten)]
    use_case_options: ProjectUseCaseOpt,
}

impl Init {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        println!("\nWelcome! This command helps you initialize a new GraphQL API project using Apollo Federation with Apollo Router.\n");

        let _ = self.use_case_options.get_or_prompt_use_case()?;

        Ok(RoverOutput::EmptySuccess)
    }
}