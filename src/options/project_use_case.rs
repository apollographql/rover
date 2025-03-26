use clap::{Parser, ValueEnum};
use std::fmt::{self, Display};
use anyhow::anyhow;
use console::Term;
use dialoguer::Select;
use serde::{Deserialize, Serialize};

use crate::{RoverResult, RoverError};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct ProjectUseCaseOpt {
    /// Filter templates by the available use case
    #[arg(long = "project_use_case", value_enum)]
    pub project_use_case: Option<ProjectUseCase>,
}

impl ProjectUseCaseOpt {
    pub fn get_or_prompt_use_case(&self) -> RoverResult<ProjectUseCase> {
        if let Some(project_use_case) = &self.project_use_case {
            Ok(project_use_case.clone())
        } else {
            let use_cases = <ProjectUseCase as ValueEnum>::value_variants();

            let selection = Select::new()
                .with_prompt("? Select use case")
                .items(use_cases)
                .default(0)
                .interact_on_opt(&Term::stderr())?;

            match selection {
                Some(index) => Ok(use_cases[index].clone()),
                None => Err(RoverError::new(anyhow!("No use case selected"))),
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, clap::ValueEnum)]
pub enum ProjectUseCase {
    Connectors,
    GraphQLTemplate,
}

impl Display for ProjectUseCase {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ProjectUseCase::*;
        let readable = match self {
            Connectors => "Connect one or more REST APIs",
            GraphQLTemplate => "Start a GraphQL API with recommended libraries",
        };
        write!(f, "{}", readable)
    }
}