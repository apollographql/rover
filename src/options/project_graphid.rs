use crate::command::init::graph_id::generation::generate_graph_id;
use crate::command::init::graph_id::validation::{validate_graph_id, GraphIdValidationError};
use crate::command::init::graph_id::errors::conversions::validation_error_to_rover_error;
use crate::command::init::graph_id::utils::random::DefaultRandomStringGenerator;
use crate::RoverResult;
use clap::arg;
use clap::Parser;
use dialoguer::Input;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct GraphIdOpt {
    #[arg(long = "graph-id")]
    pub graph_id: Option<String>,
}

impl Default for GraphIdOpt {
    fn default() -> Self {
        Self { graph_id: None }
    }
}

impl GraphIdOpt {
    pub fn get_or_prompt_graph_id(
        &self,
        project_name: &str,
    ) -> RoverResult<String> {
        // Handle the case when graph_id is provided via command line
        if let Some(ref id) = self.graph_id {
            validate_graph_id(id)?;
            return Ok(id.clone());
        }
        
        // Generate a suggested ID for the prompt
        let suggested_id = generate_graph_id(project_name, &mut DefaultRandomStringGenerator);
        
        // Enter prompt/validate loop
        self.prompt_graph_id(suggested_id)
    }

    fn prompt_graph_id(
        &self,
        suggested_id: String,
    ) -> RoverResult<String> {
        const MAX_RETRIES: usize = 3;

        for attempt in 1..=MAX_RETRIES {
            let input = self.prompt_for_input(&suggested_id)?;

            match validate_graph_id(&input) {
                Ok(()) => return Ok(input),
                Err(e) => self.handle_validation_error(e, attempt, MAX_RETRIES)?,
            }
        }

        unreachable!("Loop should have exited with return Ok or Err");
    }

    fn prompt_for_input(&self, suggested_id: &str) -> RoverResult<String> {
        let prompt = format!("Confirm or modify graph ID [{}]", suggested_id);
        let input = Input::<String>::new()
            .with_prompt(&prompt)
            .default(suggested_id.to_string())
            .allow_empty(false)
            .interact()?;

        Ok(input)
    }

    fn handle_validation_error(
        &self,
        error: GraphIdValidationError,
        attempt: usize,
        max_retries: usize,
    ) -> RoverResult<()> {
        // If last attempt, propagate the error
        if attempt == max_retries {
            return Err(validation_error_to_rover_error(error));
        }

        // Otherwise display error and signal to retry
        eprintln!("{}", error);
        eprintln!("Please try again (attempt {}/{})", attempt, max_retries);
        Ok(())
    }
}
