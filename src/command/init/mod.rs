mod config;
pub mod graph_id_operations;
mod helpers;
mod states;
mod template_operations;
mod transitions;

use crate::options::{
    GraphIdOpt, ProjectNameOpt, ProjectOrganizationOpt, ProjectTypeOpt, ProjectUseCaseOpt,
};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};
use camino::Utf8PathBuf;
use clap::Parser;
use rover_http::ReqwestService;
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
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        // Create a new ReqwestService instance for template preview
        let http_service = ReqwestService::new(None, None)?;

        let creation_confirmed_option = Welcome::new()
            .select_project_type(&self.project_type)?
            .select_organization(&self.organization)?
            .select_use_case(&self.project_use_case)?
            .enter_project_name(&self.project_name)?
            .confirm_graph_id(&self.graph_id, client_config)?
            .preview_and_confirm_creation(http_service)
            .await?;

        match creation_confirmed_option {
            Some(creation_confirmed) => {
                let project_created = creation_confirmed.create_project().await?;
                Ok(project_created.complete().success())
            }
            None => Ok(RoverOutput::EmptySuccess),
        }
    }
}

pub use states::Welcome;
