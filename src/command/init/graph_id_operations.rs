use crate::error::{RoverError, RoverErrorSuggestion};
use crate::RoverResult;
use clap::arg;
use clap::Parser;
use dialoguer::{theme::ColorfulTheme, Input};
use regex::Regex;
use serde::{Deserialize, Serialize};

const MAX_GRAPH_ID_LENGTH: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct ProjectNameOpt {
    #[arg(long = "project-name")]
    pub project_name: Option<String>,
}

#[derive(Debug)]
pub enum GraphIdValidationError {
    Empty,
    DoesNotStartWithLetter,
    ContainsInvalidCharacters,
    TooLong,
    AlreadyExists,
}

impl GraphIdValidationError {
    pub fn to_rover_error(self) -> RoverError {
        match self {
            Self::Empty => RoverError::new(
                "Graph ID cannot be empty".to_string(),
                RoverErrorSuggestion::Adhoc(
                    "Please enter a valid graph ID starting with a letter and containing only letters, numbers, underscores, and hyphens.".to_string(),
                ),
            ),
            Self::DoesNotStartWithLetter => RoverError::new(
                "Graph ID must start with a letter".to_string(),
                RoverErrorSuggestion::Adhoc(
                    "Please ensure your graph ID starts with a letter (a-z, A-Z).".to_string(),
                ),
            ),
            Self::ContainsInvalidCharacters => RoverError::new(
                "Graph ID contains invalid characters".to_string(),
                RoverErrorSuggestion::Adhoc(
                    "Graph IDs can only contain letters, numbers, underscores, and hyphens.".to_string(),
                ),
            ),
            Self::TooLong => RoverError::new(
                format!("Graph ID exceeds maximum length of {}", MAX_GRAPH_ID_LENGTH),
                RoverErrorSuggestion::Adhoc(
                    format!("Please ensure your graph ID is no longer than {} characters.", MAX_GRAPH_ID_LENGTH),
                ),
            ),
            Self::AlreadyExists => RoverError::new(
                "Graph ID already exists".to_string(),
                RoverErrorSuggestion::Adhoc(
                    "This graph ID is already in use. Please choose a different name for your GraphQL API.".to_string(),
                ),
            ),
        }
    }
}

impl ProjectNameOpt {
    pub fn get_project_name(&self) -> Option<String> {
        self.project_name.clone()
    }

    fn suggest_default_name(&self) -> String {
        "my-graphql-api".to_string()
    }

    fn validate_project_name(project_name: &str) -> Result<(), GraphIdValidationError> {
        if project_name.is_empty() {
            return Err(GraphIdValidationError::Empty);
        }

        let first_char = project_name.chars().next().unwrap();
        if !first_char.is_alphabetic() {
            return Err(GraphIdValidationError::DoesNotStartWithLetter);
        }

        let valid_pattern = Regex::new(r"^[a-zA-Z][a-zA-Z0-9_-]*$").unwrap();
        if !valid_pattern.is_match(project_name) {
            return Err(GraphIdValidationError::ContainsInvalidCharacters);
        }

        if project_name.len() > MAX_GRAPH_ID_LENGTH {
            return Err(GraphIdValidationError::TooLong);
        }

        Ok(())
    }

    async fn check_name_availability(project_name: &str) -> RoverResult<bool> {
        // TODO: REPLACE WITH API CALL
        Ok(true)
    }

    pub async fn prompt_project_name(&self) -> RoverResult<String> {
        let default = self.suggest_default_name();

        let mut input: Input<String> = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Name your GraphQL API")
            .with_initial_text(default.clone())
            .allow_empty(false);

        loop {
            let project_name = input.interact()?;

            match Self::validate_project_name(&project_name) {
                Ok(()) => match Self::check_name_availability(&project_name).await {
                    Ok(true) => return Ok(project_name),
                    Ok(false) => {
                        eprintln!("Error: This name is already in use. Please choose a different name for your GraphQL API.");
                        continue;
                    }
                    Err(e) => return Err(e),
                },
                Err(e) => {
                    eprintln!("Error: {}", e.to_rover_error().message);
                    eprintln!("Suggestion: {}", e.to_rover_error().suggestion);
                    continue;
                }
            }
        }
    }

    pub async fn get_or_prompt_project_name(&self) -> RoverResult<String> {
        // If a project name was provided via command line, validate and use it
        if let Some(name) = self.get_project_name() {
            if let Err(e) = Self::validate_project_name(&name) {
                return Err(e.to_rover_error());
            }

            match Self::check_name_availability(&name).await {
                Ok(true) => Ok(name),
                Ok(false) => Err(GraphIdValidationError::AlreadyExists.to_rover_error()),
                Err(e) => Err(e),
            }
        } else {
            self.prompt_project_name().await
        }
    }
}

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
    fn test_validate_project_name() {
        // Valid names
        assert!(ProjectNameOpt::validate_project_name("valid-id").is_ok());
        assert!(ProjectNameOpt::validate_project_name("a").is_ok());
        assert!(ProjectNameOpt::validate_project_name("valid_id_with_underscore").is_ok());
        assert!(ProjectNameOpt::validate_project_name("validIdWith123Numbers").is_ok());

        // Invalid names
        assert!(matches!(
            ProjectNameOpt::validate_project_name(""),
            Err(GraphIdValidationError::Empty)
        ));

        assert!(matches!(
            ProjectNameOpt::validate_project_name("123-invalid-start"),
            Err(GraphIdValidationError::DoesNotStartWithLetter)
        ));

        assert!(matches!(
            ProjectNameOpt::validate_project_name("invalid!chars"),
            Err(GraphIdValidationError::ContainsInvalidCharacters)
        ));

        let too_long = "a".repeat(MAX_GRAPH_ID_LENGTH + 1);
        assert!(matches!(
            ProjectNameOpt::validate_project_name(&too_long),
            Err(GraphIdValidationError::TooLong)
        ));
    }
}
