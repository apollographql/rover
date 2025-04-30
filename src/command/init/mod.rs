#[cfg(feature = "composition-js")]
mod authentication;
#[cfg(feature = "composition-js")]
mod config;
#[cfg(feature = "composition-js")]
mod graph_id;
#[cfg(feature = "composition-js")]
mod helpers;
#[cfg(feature = "composition-js")]
mod operations;
pub mod options;
#[cfg(feature = "composition-js")]
pub mod spinner;
#[cfg(feature = "composition-js")]
pub mod states;
#[cfg(feature = "composition-js")]
pub mod template_operations;
#[cfg(all(test, feature = "composition-js"))]
pub mod tests;
#[cfg(feature = "composition-js")]
pub mod transitions;

#[cfg(feature = "composition-js")]
use crate::command::init::options::{
    GraphIdOpt, ProjectNameOpt, ProjectOrganizationOpt, ProjectTypeOpt, ProjectUseCaseOpt,
};
#[cfg(feature = "composition-js")]
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};
use clap::Parser;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Parser, Clone, Serialize)]
#[clap(about = "Initialize a new graph")]
pub struct Init {
    #[cfg(feature = "composition-js")]
    #[clap(flatten)]
    project_type: ProjectTypeOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    organization: ProjectOrganizationOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    project_use_case: ProjectUseCaseOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    project_name: ProjectNameOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    graph_id: GraphIdOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    profile: ProfileOpt,

    #[clap(long, hide(true))]
    path: Option<PathBuf>,
}

impl Init {
    #[cfg(feature = "composition-js")]
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        use crate::command::init::options::ProjectType;
        use crate::command::init::states::UserAuthenticated;
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
        use rover_std::hyperlink;

        let mut err = RoverError::new(anyhow!(
            "This version of Rover does not support this command."
        ));
        if cfg!(target_env = "musl") {
            err.set_suggestion(RoverErrorSuggestion::Adhoc(format!("Unfortunately, Deno does not currently support musl architectures. You can follow along with this issue for updates on musl support: {}, for now you will need to switch to a Linux distribution (like Ubuntu or CentOS) that can run Rover's prebuilt binaries.", hyperlink("https://github.com/denoland/deno/issues/3711"))));
        }

        Err(err)
    }
}
