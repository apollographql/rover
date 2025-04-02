use crate::{RoverError, RoverResult};
use anyhow::anyhow;
use clap::{Parser, ValueEnum};
use console::Term;
use dialoguer::Select;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectTypeOpt {
    #[arg(long = "project-type", value_enum)]
    pub project_type: Option<ProjectType>,
}

impl ProjectTypeOpt {
    pub fn get_project_type(&self) -> Option<ProjectType> {
        self.project_type.clone()
    }

    pub fn prompt_project_type(&self) -> RoverResult<ProjectType> {
        let project_types = <ProjectType as ValueEnum>::value_variants();
        let selection = Select::new()
            .with_prompt("? Select project type")
            .items(&project_types)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        self.handle_project_type_selection(&project_types, selection)
    }

    fn handle_project_type_selection(&self, project_types: &[ProjectType], selection: Option<usize>) -> RoverResult<ProjectType> {
        match selection {
            Some(index) => Ok(project_types[index].clone()),
            None => Err(RoverError::new(anyhow!("No project type selected"))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, clap::ValueEnum)]
pub enum ProjectType {
    CreateNew,
    AddSubgraph,
}

impl Display for ProjectType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ProjectType::*;
        let readable = match self {
            CreateNew => "Create a new GraphQL API",
            AddSubgraph => "Add a subgraph to an existing GraphQL API",
        };
        write!(f, "{}", readable)
    }
}