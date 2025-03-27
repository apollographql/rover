use crate::{RoverError, RoverResult};
use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use console::Term;
use dialoguer::Select;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct ProjectUseCaseOpt {
    #[arg(long = "project-use-case", value_enum)]
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

            self.handle_use_case_selection(use_cases, selection)
        }
    }

    pub fn handle_use_case_selection(&self, use_cases: &[ProjectUseCase], selection: Option<usize>) -> RoverResult<ProjectUseCase> {
        match selection {
            Some(index) => Ok(use_cases[index].clone()),
            None => Err(RoverError::new(anyhow!("No use case selected"))),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_use_case_with_some_project_use_case() {
        let instance = ProjectUseCaseOpt {
            project_use_case: Some(ProjectUseCase::Connectors),
        };

        let result = instance.get_or_prompt_use_case();

        assert!(result.is_ok());
        let use_case = result.unwrap();
        assert_eq!(use_case, ProjectUseCase::Connectors);
    }

    #[test]
    fn test_handle_use_case_selection_returns_use_case_with_some_selection() {
        let instance = ProjectUseCaseOpt {
            project_use_case: None,
        };

        let use_cases = <ProjectUseCase as ValueEnum>::value_variants();
        let selection: usize = 0;
        let result = instance.handle_use_case_selection(use_cases, Some(selection));

        assert!(result.is_ok());
        let use_case = result.unwrap();
        assert_eq!(use_case, use_cases[selection].clone());
    }

    #[test]
    fn test_handle_use_case_selection_returns_error_with_none_selection() {
        let instance = ProjectUseCaseOpt {
            project_use_case: None,
        };

        let use_cases = <ProjectUseCase as ValueEnum>::value_variants();
        let result = instance.handle_use_case_selection(use_cases, None);

        assert!(result.is_err());
    }
}
