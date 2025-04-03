use crate::RoverResult;
use clap::arg;
use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectNameOpt {
    #[arg(long = "project-name")]
    pub project_name: Option<String>,
}

// TODO: Include Taylor's logic for generating a project name
impl ProjectNameOpt {
    pub fn get_project_name(&self) -> Option<String> {
        self.project_name.clone()
    }

    pub fn prompt_project_name(&self) -> RoverResult<String> {
        // TODO: Include Taylor's prompt, validation, and return project name logic here
        let _prompt = "Name your GraphQL API";
        let default = self.suggest_default_name();

        // TODO: Include Taylor's logic for generating a project name

        Ok(default)
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

// TODO: Add tests for interactive prompts and sad paths
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_project_name_with_preset_value() {
        let instance = ProjectNameOpt {
            project_name: Some("my-project".to_string()),
        };

        let result = instance.get_project_name();
        assert_eq!(result, Some("my-project".to_string()));
    }

    #[test]
    fn test_suggest_default_name() {
        let instance = ProjectNameOpt { project_name: None };
        let default_name = instance.suggest_default_name();

        assert_eq!(default_name, "my-graphql-api");
    }

    #[test]
    fn test_prompt_project_name() {
        let instance = ProjectNameOpt { project_name: None };
        let result = instance.prompt_project_name();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "my-graphql-api");
    }

    #[test]
    fn test_get_or_prompt_project_name_with_no_value() {
        let instance = ProjectNameOpt { project_name: None };
        let result = instance.get_or_prompt_project_name();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "my-graphql-api");
    }

    // Default trait implementation tests

    #[test]
    fn test_default_trait() {
        let default_instance = ProjectNameOpt::default();
        assert_eq!(default_instance.project_name, None);
    }

    // Derived trait tests (Debug, Clone, etc.)

    #[test]
    fn test_debug_trait() {
        let instance = ProjectNameOpt {
            project_name: Some("test-project".to_string()),
        };
        // Check that Debug formatting doesn't panic and has the expected content
        let debug_str = format!("{:?}", instance);
        assert!(debug_str.contains("test-project"));
    }

    #[test]
    fn test_clone_trait() {
        let original = ProjectNameOpt {
            project_name: Some("clone-test".to_string()),
        };
        let cloned = original.clone();

        assert_eq!(original.project_name, cloned.project_name);
    }
}
