use clap::arg;
use serde::{Deserialize, Serialize};
use clap::Parser;
use crate::RoverResult;
use dialoguer::Input;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct GraphIdOpt {
    #[arg(long = "graph-id")]
    pub graph_id: Option<String>,
}

impl GraphIdOpt {
    pub fn get_graph_id(&self) -> Option<String> {
        self.graph_id.clone()
    }
    
    pub fn suggest_graph_id(project_name: &str) -> String {
        format!("{}-graph@current", project_name)
    }
    
    pub fn prompt_for_graph_id(suggested_id: &str) -> RoverResult<String> {
        let graph_id = Input::<String>::new()
            .with_prompt(format!("Confirm or modify graph ID (start with a letter and use only letters, numbers, and dashes)"))
            .default(suggested_id.to_string())
            .validate_with(|input: &String| -> Result<(), &str> {
                if input.is_empty() {
                    return Err("Graph ID cannot be empty");
                }
                Ok(())
            })
            .interact()?;
        
        Ok(graph_id)
    }
    
    pub fn get_or_prompt_graph_id(&self, project_name: &str) -> RoverResult<String> {
        if let Some(id) = self.get_graph_id() {
            // TODO: Validate graph ID format
            return Ok(id);
        }
        
        let suggested_id = Self::suggest_graph_id(project_name);
        
        let graph_id = Self::prompt_for_graph_id(&suggested_id)?;
        
        Ok(graph_id)
    }
}