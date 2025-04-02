use crate::command::init::config::ProjectConfig;
use itertools::Itertools;
use rover_std::prompt::prompt_confirm_default_yes;
use std::collections::HashSet;

pub fn display_welcome_message() {
    println!("\nWelcome! This command helps you initialize a federated GraphQL API in your current directory.");
    println!("\nTo learn more about init and each use case, run `rover init -h` or visit https://www.apollographql.com/docs/rover/commands/init");
}

pub fn prompt_confirm_project_creation(config: &ProjectConfig, artifacts: Option<&[String]>) -> std::io::Result<bool> {
    println!("\nYou are about to create a project with the following settings:");
    println!("Organization: {}", config.organization);
    println!("Project name: {}", config.project_name);
    println!("Graph ID: {}", config.graph_id);
    
    // If we have artifacts, display them as well
    if let Some(artifact_list) = artifacts {
        println!("\nThe following files will be created:");
        let top_level_artifacts: HashSet<_> = artifact_list
            .iter()
            .filter(|path| path.matches('/').count() <= 1)
            .sorted()
            .collect();

        for artifact in top_level_artifacts {
            println!("• {}", artifact);
        }
    }
    
    println!();
    prompt_confirm_default_yes("Proceed with creation?")
}