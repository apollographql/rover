use crate::RoverResult;
use clap::arg;
use clap::Parser;
use dialoguer::Input;
use serde::{Deserialize, Serialize};

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
            .with_prompt("Confirm or modify graph ID (start with a letter and use only letters, numbers, and dashes)".to_string())
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

// TODO: Add tests for interactive prompts and sad paths
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_graph_id_with_preset_value() {
        let instance = GraphIdOpt {
            graph_id: Some("my-graph".to_string()),
        };

        let result = instance.get_graph_id();
        assert_eq!(result, Some("my-graph".to_string()));
    }

    #[test]
    fn test_get_graph_id_with_no_value() {
        let instance = GraphIdOpt { graph_id: None };
        let result = instance.get_graph_id();
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_or_prompt_graph_id_with_preset_value() {
        let instance = GraphIdOpt {
            graph_id: Some("custom-graph-id".to_string()),
        };

        let result = instance.get_or_prompt_graph_id("project-name");

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "custom-graph-id");
    }

    // Default trait implementation tests

    #[test]
    fn test_default_trait() {
        let default_instance = GraphIdOpt::default();
        assert_eq!(default_instance.graph_id, None);
    }

    // Derived trait tests (Debug, Clone, etc.)

    #[test]
    fn test_debug_trait() {
        let instance = GraphIdOpt {
            graph_id: Some("test-graph".to_string()),
        };

        // Check that Debug formatting doesn't panic and includes the expected content
        let debug_str = format!("{:?}", instance);
        assert!(debug_str.contains("test-graph"));
    }

    #[test]
    fn test_clone_trait() {
        let original = GraphIdOpt {
            graph_id: Some("clone-test-graph".to_string()),
        };
        let cloned = original.clone();

        // Ensure the cloned instance has the same data
        assert_eq!(original.graph_id, cloned.graph_id);
    }
}
