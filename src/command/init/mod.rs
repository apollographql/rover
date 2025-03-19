mod connectors;

use crate::command::init::EditorFamily::VSCode;
use crate::{RoverOutput, RoverResult};
use clap::Parser;
use serde::Serialize;
use strum_macros::EnumIter;
use tower::{ServiceBuilder, ServiceExt};
use rover_http::ReqwestService;
use rover_std::prompt::prompt_confirm_default_yes;
use crate::command::init::connectors::ConnectorProject;

#[derive(Debug, Serialize, Parser)]
pub struct Init {}

impl Init {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        println!("\nWelcome! This command helps you initialize a new GraphQL API project using Apollo Federation with Apollo Router.\n");

        // TODO: get from user input
        let editor = VSCode;


        let request_service = ReqwestService::builder().build()?;
        let mut service = ServiceBuilder::new().service(request_service);
        let http_service = service.ready().await?;

        //TODO: Prompt the user the type of project

        //a connectors project in this case
        let mut connector = ConnectorProject::new(editor);

        connector.fetch_repo(http_service).await?;
        connector.display_files()?;

        if prompt_confirm_default_yes("Proceed with creation?")? {
            connector.write_template(".")?;
        }
        
        Ok(RoverOutput::EmptySuccess)
    }
}

#[derive(EnumIter, Eq, PartialEq, Debug, Clone, Copy)]
enum EditorFamily {
    VSCode,
    Jetbrains,
}

impl EditorFamily {
    fn path(&self) -> &'static str {
        match self {
            EditorFamily::VSCode => ".vscode/",
            EditorFamily::Jetbrains => ".idea/",
        }
    }
}

// the idea here is that we can create a ConnectorsProject or a template but call the same fn
// from init and expect the same behavior.
pub trait InitProjectActions {
    //Call this to fetch the repository and hold it in memory
    async fn fetch_repo(&mut self, http_service: &mut ReqwestService) -> RoverResult<()>;

    //making the project itself be in charge of displaying the files it will create
    fn display_files(&self) -> RoverResult<()>;

    // write files to the target dir
    fn write_template(&self, target_path: &str) -> RoverResult<()>;
}

