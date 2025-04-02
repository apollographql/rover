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
    project_type_opt: ProjectTypeOpt,
    
    #[clap(flatten)]
    organization_opt: ProjectOrganizationOpt,
    
    #[clap(flatten)]
    project_use_case_opt: ProjectUseCaseOpt,
    
    #[clap(flatten)]
    project_name_opt: ProjectNameOpt,

    #[clap(flatten)]
    graph_id_opt: GraphIdOpt,

    #[clap(long, hide(true))]
    path: Option<Utf8PathBuf>,
}

impl Init {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        let welcome = Welcome::new();
        
        let project_type_selected = welcome.select_project_type(&self.project_type_opt)?;
        
        let organization_selected = project_type_selected.select_organization(&self.organization_opt)?;
        
        let use_case_selected = organization_selected.select_use_case(&self.project_use_case_opt)?;
        
        let project_named = use_case_selected.enter_project_name(&self.project_name_opt)?;
        
        let graph_id_confirmed = project_named.confirm_graph_id(&self.graph_id_opt)?;
        
        // Create a new ReqwestService instance for template preview
        let http_service = ReqwestService::new(None, None)?;
        let creation_confirmed = match graph_id_confirmed.preview_and_confirm_creation(http_service.clone()).await? {
            Some(confirmed) => confirmed,
            None => return Ok(RoverOutput::EmptySuccess), 
        };
        
        // Reuse the same http_service for project creation
        let project_created = creation_confirmed.create_project(http_service).await?;
        
        let completed = project_created.complete();
        
        let output = completed.success();
        
        Ok(output)
    }
}

pub use states::Welcome;
