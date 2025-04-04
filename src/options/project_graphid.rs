use crate::command::GraphIdOperations;
use crate::command::GraphIdValidationError;
use crate::utils::client::StudioClientConfig;
use crate::RoverResult;
use crate::{RoverError, RoverErrorSuggestion};
use anyhow::anyhow;
use clap::arg;
use clap::Parser;
use dialoguer::Input;
use rand::Rng;
use rover_client::operations::init::check;
use rover_client::operations::init::CheckGraphIdAvailabilityInput;
use serde::{Deserialize, Serialize};

use crate::options::ProfileOpt;

const MAX_GRAPH_ID_LENGTH: usize = 64;
const GRAPH_ID_MAX_CHAR: usize = 27;

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct GraphIdOpt {
    #[arg(long = "graph-id")]
    pub graph_id: Option<String>,

    #[clap(flatten)]
    pub profile: ProfileOpt,
}

impl GraphIdOpt {
    fn prompt_graph_id(&self, project_name: &str) -> RoverResult<String> {
        let suggested_id = generate_graph_id(project_name);
        const MAX_RETRIES: usize = 3;

        for attempt in 1..=MAX_RETRIES {
            let graph_id = self.get_graph_id_input(&suggested_id)?;

            match self.validate_and_verify_graph_id(&graph_id, attempt, MAX_RETRIES) {
                Ok(valid_id) => return Ok(valid_id),
                Err(e) if attempt == MAX_RETRIES => return Err(e),
                Err(_) => continue, // Try again if not the last attempt
            }
        }

        unreachable!("Loop should have exited with return Ok or Err");
    }

    fn get_graph_id_input(&self, suggested_id: &str) -> RoverResult<String> {
        let prompt = format!("Confirm or modify graph ID [{}]", suggested_id);

        let input = Input::<String>::new()
            .with_prompt(&prompt)
            .default(suggested_id.to_string())
            .allow_empty(false);

        let graph_id = input.interact()?;
        Ok(graph_id)
    }

    fn validate_and_verify_graph_id(
        &self,
        graph_id: &str,
        attempt: usize,
        max_retries: usize,
    ) -> RoverResult<String> {
        match GraphIdOperations::validate_graph_id(graph_id) {
            Err(validation_error) => {
                self.handle_validation_error(validation_error, attempt, max_retries)?;
                Err(self.create_retry_error(attempt, max_retries))
            }
            Ok(()) => Ok(graph_id.to_string()),
        }
    }

    fn handle_validation_error(
        &self,
        validation_error: GraphIdValidationError,
        attempt: usize,
        max_retries: usize,
    ) -> RoverResult<()> {
        match validation_error {
            GraphIdValidationError::Empty => {
                eprintln!("Graph ID cannot be empty.");
            }
            GraphIdValidationError::DoesNotStartWithLetter => {
                eprintln!("Graph ID must start with a letter.");
            }
            GraphIdValidationError::ContainsInvalidCharacters => {
                eprintln!("Graph ID can only contain letters, numbers, underscores, and hyphens.");
            }
            GraphIdValidationError::TooLong => {
                eprintln!(
                    "Graph ID exceeds maximum length of {}.",
                    MAX_GRAPH_ID_LENGTH
                );
            }
            GraphIdValidationError::AlreadyExists => {
                eprintln!("This graph ID is already in use. Please choose a different name.");
            }
        }

        // If this is the last attempt, exit with an error
        if attempt == max_retries {
            eprintln!("Maximum retry attempts reached. Command aborted.");

            // Return a specific error for maximum retries
            return Err(RoverError::new(anyhow!(
                "Failed to provide a valid graph ID after {} attempts.",
                max_retries
            )));
        }

        Ok(())
    }

    fn create_retry_error(&self, attempt: usize, max_retries: usize) -> RoverError {
        RoverError::new(anyhow!(
            "Invalid graph ID (attempt {}/{}). Please try again.",
            attempt,
            max_retries
        ))
    }

    pub async fn get_or_prompt_graph_id(
        &self,
        client_config: StudioClientConfig,
        project_name: &str,
    ) -> RoverResult<String> {
        // If a graph ID was provided via command line, validate and use it
        if let Some(graph_id) = &self.graph_id {
            // Step 1: Validate the format of the provided graph ID
            if let Err(e) = GraphIdOperations::validate_graph_id(graph_id) {
                return Err(e.to_rover_error());
            }
    
            // Step 2: Check if the graph ID already exists
            let exists = self.check_if_graph_id_exists(client_config, graph_id).await?;
            
            // This is the corrected part - we should error if the graph ID exists
            if exists {
                return Err(RoverError::new(anyhow!(
                    "The graph ID '{}' is already in use.",
                    graph_id
                ))
                .with_suggestion(RoverErrorSuggestion::Adhoc(
                    "Please choose a different graph ID and try again.".to_string(),
                )));
            }
    
            // Return a clone of the String to satisfy ownership requirements
            return Ok(graph_id.clone());
        }
    
        // If no graph ID was provided, prompt the user
        self.prompt_graph_id(project_name)
    }

    /// Checks if the provided graph ID already exists in the Apollo graph registry.
    ///
    /// Returns `true` if the graph ID exists, `false` otherwise.
    async fn check_if_graph_id_exists(
        &self,
        client_config: StudioClientConfig,
        graph_id: &str,
    ) -> RoverResult<bool> {
        // Create a StudioClient from the config
        let client = client_config.get_authenticated_client(&self.profile)?;
        
        let client_call = check::run(
            CheckGraphIdAvailabilityInput {
                graph_id: graph_id.to_string(),
            },
            &client,
        )
        .await?;
        Ok(client_call.available)
    }
}

impl Default for GraphIdOpt {
    fn default() -> Self {
        Self {
            graph_id: None,
            profile: ProfileOpt {
                profile_name: String::new(),
            },
        }
    }
}

// HELPERS: SHOULD MOVE TO THEIR OWN FILE
// Generate a random string of 7 characters (for graph ID suggestions)
fn generate_unique_string() -> String {
    // Generate a random number between 0 and 1, convert to base 36, and take substring
    let mut rng = rand::rng();
    let random_val: f64 = rng.random();
    random_val.to_string()[2..]
        .chars()
        .map(|c| if c == '.' { 'a' } else { c })
        .collect::<String>()[..7]
        .to_string()
}

// Slugify a string for use as a graph ID
fn slugify(input: &str) -> String {
    let mut result = input.to_lowercase().replace(' ', "-");

    // Replace consecutive hyphens with a single hyphen
    while result.contains("--") {
        result = result.replace("--", "-");
    }

    // Remove leading and trailing hyphens
    result = result
        .trim_start_matches('-')
        .trim_end_matches('-')
        .to_string();

    result
}

fn generate_graph_id(graph_name: &str) -> String {
    // Step 1: Slugify the graph name with strict mode
    let mut slugified_name = slugify(graph_name);

    // Step 2: Remove non-alphabetic characters from the beginning
    let alphabetic_start_index = slugified_name
        .chars()
        .position(|c| c.is_alphabetic())
        .unwrap_or(slugified_name.len());
    slugified_name = slugified_name[alphabetic_start_index..].to_string();

    // Step 3: Calculate how much space to reserve for the unique string
    let unique_string = generate_unique_string();
    let unique_string_length = unique_string.len() + 1;

    // Step 4: Get the appropriate slice of slugified name
    let max_name_length = if GRAPH_ID_MAX_CHAR > unique_string_length {
        GRAPH_ID_MAX_CHAR - unique_string_length
    } else {
        0
    };

    let name_part = slugified_name[..slugified_name.len().min(max_name_length)].to_string();

    // Step 5: Add "id" if name is empty
    let name_part = if name_part.is_empty() {
        "id".to_string()
    } else {
        name_part
    };

    // Step 6: Append unique string if provided
    let result = format!("{}-{}", name_part, unique_string);

    // Step 7: Slugify again and ensure max length
    let final_result = slugify(&result);
    final_result[..final_result.len().min(GRAPH_ID_MAX_CHAR)].to_string()
}

// TODO: Add tests for interactive prompts and sad paths
#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::ProfileOpt;

    // Default trait implementation tests

    // Derived trait tests (Debug, Clone, etc.)

    #[test]
    fn test_debug_trait() {
        let instance = GraphIdOpt {
            graph_id: Some("test-graph".to_string()),
            profile: ProfileOpt::default(),
        };

        // Check that Debug formatting doesn't panic and includes the expected content
        let debug_str = format!("{:?}", instance);
        assert!(debug_str.contains("test-graph"));
    }

    #[test]
    fn test_clone_trait() {
        let original = GraphIdOpt {
            graph_id: Some("clone-test-graph".to_string()),
            profile: ProfileOpt::default(),
        };
        let cloned = original.clone();

        // Ensure the cloned instance has the same data
        assert_eq!(original.graph_id, cloned.graph_id);
    }
}
