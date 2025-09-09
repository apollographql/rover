#[cfg(feature = "composition-js")]
mod authentication;
#[cfg(feature = "composition-js")]
mod config;
#[cfg(feature = "composition-js")]
mod graph_id;
#[cfg(feature = "composition-js")]
mod helpers;
#[cfg(feature = "composition-js")]
mod mcp_operations;
#[cfg(feature = "composition-js")]
mod operations;
pub mod options;
#[cfg(feature = "composition-js")]
pub mod states;
#[cfg(feature = "composition-js")]
mod template_fetcher;
#[cfg(feature = "composition-js")]
pub mod template_operations;
#[cfg(all(test, feature = "composition-js"))]
pub mod tests;
#[cfg(feature = "composition-js")]
pub mod transitions;
#[cfg(feature = "composition-js")]
use crate::RoverError;
#[cfg(feature = "composition-js")]
use crate::command::init::options::ProjectTemplateOpt;
#[cfg(feature = "composition-js")]
use crate::command::init::options::{
    GraphIdOpt, ProjectNameOpt, ProjectOrganizationOpt, ProjectType, ProjectTypeOpt,
    ProjectUseCaseOpt,
};
#[cfg(feature = "composition-js")]
use crate::error::RoverErrorSuggestion;
#[cfg(feature = "composition-js")]
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};
use clap::Parser;
#[cfg(feature = "composition-js")]
use rover_client::RoverClientError;
#[cfg(feature = "composition-js")]
use rover_std::hyperlink;
use serde::Serialize;
use std::path::PathBuf;

#[cfg(feature = "composition-js")]
pub use template_fetcher::InitTemplateFetcher;

#[cfg(feature = "composition-js")]
use transitions::{CreateProjectResult, RestartReason};

#[derive(Debug, Parser, Clone, Serialize)]
#[clap(about = "Initialize a new graph")]
pub struct Init {
    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    project_template: ProjectTemplateOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
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
        use crate::command::init::states::UserAuthenticated;
        use helpers::display_use_template_message;
        let welcome = UserAuthenticated::new()
            .check_authentication(&client_config, &self.profile)
            .await?;

        let project_type_selected = welcome.select_project_type(&self.project_type, &self.path)?;

        // Early return for AddSubgraph case
        if project_type_selected.project_type == ProjectType::AddSubgraph {
            display_use_template_message();
            return Ok(RoverOutput::EmptySuccess);
        }

        // Handle new project creation flow
        let use_case_selected = match project_type_selected
            .select_organization(&self.organization, &self.profile, &client_config)
            .await?
            .select_use_case(&self.project_use_case)?
        {
            Some(use_case) => use_case,
            None => return Ok(RoverOutput::EmptySuccess),
        };

        let creation_confirmed = match use_case_selected
            .select_template(&self.project_template)
            .await?
            .enter_project_name(&self.project_name)?
            .confirm_graph_id(&self.graph_id)?
            .preview_and_confirm_creation()
            .await?
        {
            Some(confirmed) => confirmed,
            None => return Ok(RoverOutput::EmptySuccess),
        };

        let project_created = creation_confirmed
            .create_project(&client_config, &self.profile)
            .await?;

        // Handle project creation result
        if let CreateProjectResult::Created(project) = project_created {
            return Ok(project.complete().success());
        }

        // Handle restart loop
        if let CreateProjectResult::Restart {
            state: mut current_project,
            reason: _,
        } = project_created
        {
            const MAX_RETRIES: u8 = 3;

            for attempt in 0..MAX_RETRIES {
                if attempt >= MAX_RETRIES {
                    let suggestion = RoverErrorSuggestion::Adhoc(
                        format!(
                            "If the issue persists, please contact support at {}.",
                            hyperlink("https://support.apollographql.com")
                        )
                        .to_string(),
                    );
                    let error = RoverError::from(RoverClientError::MaxRetriesExceeded {
                        max_retries: MAX_RETRIES,
                    })
                    .with_suggestion(suggestion);
                    return Err(error);
                }

                let graph_id_confirmed = current_project.confirm_graph_id(&self.graph_id)?;
                let creation_confirmed =
                    match graph_id_confirmed.preview_and_confirm_creation().await? {
                        Some(confirmed) => confirmed,
                        None => return Ok(RoverOutput::EmptySuccess),
                    };

                match creation_confirmed
                    .create_project(&client_config, &self.profile)
                    .await?
                {
                    CreateProjectResult::Created(project) => {
                        return Ok(project.complete().success());
                    }
                    CreateProjectResult::Restart {
                        state: project_named,
                        reason,
                    } => match reason {
                        RestartReason::FullRestart => {
                            let welcome = UserAuthenticated::new()
                                .check_authentication(&client_config, &self.profile)
                                .await?;
                            welcome.select_project_type(&self.project_type, &self.path)?;
                            return Ok(RoverOutput::EmptySuccess);
                        }
                        _ => {
                            current_project = project_named;
                            continue;
                        }
                    },
                }
            }
        }

        Ok(RoverOutput::EmptySuccess)
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
