mod config;
mod helpers;
mod states;
mod template_operations;
mod transitions;

use crate::options::{
    GraphIdOpt, ProjectNameOpt, ProjectOrganizationOpt, ProjectTypeOpt, ProjectUseCaseOpt,
};
mod project;

use crate::options::{ProjectUseCase, ProjectUseCaseOpt};
use crate::{RoverOutput, RoverResult};
use clap::Parser;
use rover_http::ReqwestService;
use dialoguer::Input;
use project::Project;
use serde::Serialize;

#[cfg(test)]
pub mod tests;

#[derive(Debug, Parser, Clone, Serialize)]
#[clap(about = "Initialize a new GraphQL API project")]
pub struct Init {
    #[clap(flatten)]
    project_type: ProjectTypeOpt,

    #[clap(flatten)]
    organization: ProjectOrganizationOpt,

    #[clap(flatten)]
    project_use_case: ProjectUseCaseOpt,

    #[clap(flatten)]
    project_name: ProjectNameOpt,

    #[clap(flatten)]
    graph_id: GraphIdOpt,

    #[clap(long, hide(true))]
    path: Option<Utf8PathBuf>,
}

impl Init {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        // Create a new ReqwestService instance for template preview
        let http_service = ReqwestService::new(None, None)?;

        let creation_confirmed_option = Welcome::new()
            .select_project_type(&self.project_type)?
            .select_organization(&self.organization)?
            .select_use_case(&self.project_use_case)?
            .enter_project_name(&self.project_name)?
            .confirm_graph_id(&self.graph_id)?
            .preview_and_confirm_creation(http_service)
            .await?;

        match creation_confirmed_option {
            Some(creation_confirmed) => {
                let project_created = creation_confirmed.create_project().await?;
                Ok(project_created.complete().success())
            }
            None => Ok(RoverOutput::EmptySuccess),
        }
        let use_case = self.use_case_options.get_or_prompt_use_case()?;

        match use_case {
            ProjectUseCase::GraphQLTemplate => println!("\nComing soon!\n"),
            ProjectUseCase::Connectors => {
                let _project_name =  Project::prompt_for_valid_project_name()?;
                println!("\nComing soon!\n")
            },
        }

        Ok(RoverOutput::EmptySuccess)
    }
}

pub use states::Welcome;
fn is_valid_string(input: &str, max_length: usize, allowed_chars: &str) -> bool {
    // Check length
    if input.len() > max_length {
        return false;
    }

    // Check characters
    for char in input.chars() {
        if !allowed_chars.contains(char) {
            return false;
        }
    }

    true
}
