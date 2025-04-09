use crate::command::init::graph_id::{generate_unique_graph_id, validate_and_check_availability};
use crate::{RoverError, RoverResult};
use clap::arg;
use clap::Parser;
use dialoguer::Input;
use rover_client::blocking::StudioClient;
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
    pub async fn get_or_prompt_graph_id(
        &self,
        client: &StudioClient,
        project_name: &str,
        organization_id: &str,
    ) -> RoverResult<String> {
        // Handle the case when graph_id is provided via command line
        if let Some(ref id) = self.graph_id {
            validate_and_check_availability(id, organization_id, client).await?;
            return Ok(id.clone());
        }
        
        // Generate a suggested ID for the prompt
        let suggested_id = generate_unique_graph_id(project_name);
        
        // Enter prompt/validate loop
        self.prompt_graph_id(suggested_id, organization_id, client).await
    }

    async fn prompt_graph_id(
        &self,
        suggested_id: String,
        organization_id: &str,
        client: &StudioClient,
    ) -> RoverResult<String> {
        const MAX_RETRIES: usize = 3;

        for attempt in 1..=MAX_RETRIES {
            let input = self.prompt_for_input(&suggested_id)?;

            match validate_and_check_availability(&input, organization_id, client).await {
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
        error: RoverError,
        attempt: usize,
        max_retries: usize,
    ) -> RoverResult<()> {
        // If last attempt, propagate the error
        if attempt == max_retries {
            return Err(error);
        }

        // Otherwise display error and signal to retry
        eprintln!("Error: {}", error);
        eprintln!("Please try again (attempt {}/{})", attempt, max_retries);
        Ok(())
    }
}
