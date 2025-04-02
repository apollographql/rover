mod config;
mod states;
mod helpers;
mod transitions;
mod template_operations;

use crate::options::{ProjectTypeOpt, ProjectUseCaseOpt, ProjectOrganizationOpt, ProjectNameOpt, GraphIdOpt};
use crate::{RoverOutput, RoverResult};
use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;
use rover_http::ReqwestService;

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
        let welcome = Welcome::new();
        
        let project_type_selected = welcome.select_project_type(&self.project_type)?;
        
        let organization_selected = project_type_selected.select_organization(&self.organization)?;
        
        let use_case_selected = organization_selected.select_use_case(&self.project_use_case)?;
        
        let project_named = use_case_selected.enter_project_name(&self.project_name)?;
        
        let graph_id_confirmed = project_named.confirm_graph_id(&self.graph_id)?;
        
        // Create a new ReqwestService instance for template preview
        let http_service = ReqwestService::new(None, None)?;
        let creation_confirmed = match graph_id_confirmed.preview_and_confirm_creation(http_service.clone()).await? {
            Some(confirmed) => confirmed,
            None => return Ok(RoverOutput::EmptySuccess), 
        };
        
        // Reuse the same http_service for project creation
        let project_created = creation_confirmed.create_project().await?;
        
        let completed = project_created.complete();
        
        let output = completed.success();
        
        Ok(output)
    }
}

pub use states::Welcome;
