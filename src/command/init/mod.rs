mod config;
pub mod graph_id;
mod helpers;
#[cfg(feature = "composition-js")]
mod operations;
pub mod options;
pub mod spinner;
pub mod states;
#[cfg(feature = "composition-js")]
pub mod template_operations;
#[cfg(feature = "composition-js")]
pub mod transitions;
use std::path::PathBuf;

use crate::command::init::options::{
    GraphIdOpt, ProjectNameOpt, ProjectOrganizationOpt, ProjectTypeOpt, ProjectUseCaseOpt,
};
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};
use clap::Parser;
use serde::Serialize;

#[derive(Debug, Parser, Clone, Serialize)]
#[clap(about = "Initialize a new graph")]
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
    #[cfg(feature = "composition-js")]
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        use crate::command::init::options::ProjectType;
        use helpers::display_use_template_message;
        use rover_http::ReqwestService;

        // Create a new ReqwestService instance for template preview
        let http_service = ReqwestService::new(None, None)?;

        let welcome = UserAuthenticated::new()
            .check_authentication(&client_config, &self.profile)
            .await?;

        let project_type_selected = welcome.select_project_type(&self.project_type, &self.path)?;

        match project_type_selected.project_type {
            ProjectType::CreateNew => {
                let use_case_selected_option = project_type_selected
                    .select_organization(&self.organization, &self.profile, &client_config)
                    .await?
                    .select_use_case(&self.project_use_case)?;

                match use_case_selected_option {
                    Some(use_case_selected) => {
                        let creation_confirmed_option = use_case_selected
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
                    None => Ok(RoverOutput::EmptySuccess),
                }
            }
            ProjectType::AddSubgraph => {
                display_use_template_message();
                Ok(RoverOutput::EmptySuccess)
            }
        }
    }

    #[cfg(not(feature = "composition-js"))]
    pub async fn run(&self, _client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        use crate::RoverError;
        use crate::RoverErrorSuggestion;
        use anyhow::anyhow;
        use rover_std::Style;

        let mut err = RoverError::new(anyhow!(
            "This version of Rover does not support this command."
        ));
        err.set_suggestion(RoverErrorSuggestion::Adhoc(format!(
            "It looks like you are running a Rover binary that does not have the ability to run `{}`, please try re-installing.",
            Style::Command.paint("rover init")
        )));
        Err(err)
    }
}

#[cfg_attr(not(feature = "composition-js"), allow(dead_code))]
use states::UserAuthenticated;
