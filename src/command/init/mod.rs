mod config;
pub mod graph_id;
mod helpers;
mod operations;
pub mod spinner;
pub mod states;
pub mod template_operations;
pub mod transitions;
use std::path::PathBuf;

use crate::options::{
    GraphIdOpt, ProfileOpt, ProjectNameOpt, ProjectOrganizationOpt, ProjectTypeOpt,
    ProjectUseCaseOpt,
};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};
use clap::Parser;
use helpers::display_use_template_message;
use rover_http::ReqwestService;
use serde::Serialize;

#[derive(Debug, Parser, Clone, Serialize)]
#[clap(about = "Initialize a new graph project")]
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

    #[clap(flatten)]
    profile: ProfileOpt,

    #[clap(long, hide(true))]
    path: Option<PathBuf>,
}

impl Init {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        // Create a new ReqwestService instance for template preview
        let http_service = ReqwestService::new(None, None)?;

        let welcome = UserAuthenticated::new()
            .check_authentication(&client_config, &self.profile)
            .await?;

        let project_type_selected = welcome.select_project_type(&self.project_type, &self.path)?;

        match project_type_selected.project_type {
            crate::options::ProjectType::CreateNew => {
                let creation_confirmed_option = project_type_selected
                    .select_organization(&self.organization, &self.profile, &client_config)
                    .await?
                    .select_use_case(&self.project_use_case)?
                    .enter_project_name(&self.project_name)?
                    .confirm_graph_id(&self.graph_id)?
                    .preview_and_confirm_creation(http_service)
                    .await?;

                match creation_confirmed_option {
                    Some(creation_confirmed) => {
                        let project_created = creation_confirmed
                            .create_project(&client_config, &self.profile)
                            .await?;
                        Ok(project_created.complete().success())
                    }
                    None => Ok(RoverOutput::EmptySuccess),
                }
            }
            crate::options::ProjectType::AddSubgraph => {
                display_use_template_message();
                Ok(RoverOutput::EmptySuccess)
            }
        }
    }
}

use states::UserAuthenticated;
