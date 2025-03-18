mod connectors;

use std::string::ToString;
use clap::Parser;
use console::Term;
use serde::Serialize;
use strum_macros::EnumIter;
use crate::{RoverOutput, RoverResult};
use crate::command::init::connectors::{fetch_repo, CREATE_PROMPT};
use crate::command::init::EditorFamily::VSCode;
use crate::utils::client::StudioClientConfig;

#[derive(Debug, Serialize, Parser)]
pub struct Init {
}

#[derive(EnumIter,Debug)]
enum EditorFamily {
    VSCode,
    Jetbrains
}

impl EditorFamily {
    fn path(&self) -> &'static str {
        match self {
            VSCode => ".vscode",
            Jetbrains => ".idea",
        }
    }
}

impl Init {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        println!("\nWelcome! This command helps you initialize a new GraphQL API project using Apollo Federation with Apollo Router.\n");
        println!("{}", CREATE_PROMPT);

        // TODO: get from user input
        let editor = VSCode;

        let extracted_files = fetch_repo(client_config, editor).await?;



        extracted_files.iter().for_each(|t| println!("  • {}", t.0.to_string()));
        Ok(RoverOutput::EmptySuccess)
    }
}
