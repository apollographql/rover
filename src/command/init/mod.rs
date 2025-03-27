use crate::options::{ProjectUseCase, ProjectUseCaseOpt};
use crate::{RoverOutput, RoverResult};
use clap::Parser;
use serde::Serialize;

#[derive(Debug, Serialize, Parser)]
pub struct Init {
    #[clap(flatten)]
    use_case_options: ProjectUseCaseOpt,
}

impl Init {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        println!("\nWelcome! This command helps you initialize a new GraphQL API project using Apollo Federation with Apollo Router.\n");

        let use_case = self.use_case_options.get_or_prompt_use_case()?;
        match use_case {
            ProjectUseCase::Connectors => println!("\nComing soon!\n"),
            ProjectUseCase::GraphQLTemplate => println!("\nComing soon!\n"),
        }

        Ok(RoverOutput::EmptySuccess)
    }
}
