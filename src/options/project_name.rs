
use crate::RoverResult;
use serde::{Deserialize, Serialize};
use clap::Parser;
use dialoguer::Input;
use clap::arg;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectNameOpt {
    #[arg(long = "project-name")]
    pub project_name: Option<String>,
}

impl ProjectNameOpt {
    pub fn get_project_name(&self) -> Option<String> {
        self.project_name.clone()
    }
    
    pub fn prompt_project_name(&self) -> RoverResult<String> {
      let prompt = "Name your GraphQL API";
      let default = self.suggest_default_name();
      
      let project_name = Input::<String>::new()
          .with_prompt(prompt)
          .default(default.clone())
          .interact()?;
      
      Ok(project_name)
  }
    
    fn suggest_default_name(&self) -> String {
        "my-graphql-api".to_string()
    }
    
    pub fn get_or_prompt_project_name(&self) -> RoverResult<String> {
        // If a project name was provided via command line, validate and use it
        if let Some(name) = self.get_project_name() {
            return Ok(name);
        }
        
        self.prompt_project_name()
    }
}