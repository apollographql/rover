use crate::RoverResult;
use crate::command::init::graph_id::{
    GraphId, errors::conversions::validation_error_to_rover_error, generation::generate_graph_id,
    utils::random::DefaultRandomStringGenerator, validation::GraphIdValidationError,
};
use clap::Parser;
use clap::arg;
use dialoguer::Input;
use rover_std::Style;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, Parser, Default)]
pub struct GraphIdOpt {
    #[arg(long = "graph-id", short = 'g')]
    pub graph_id: Option<String>,
}

impl GraphIdOpt {
    pub fn get_or_prompt_graph_id(&self, project_name: &str) -> RoverResult<GraphId> {
        // Handle the case when graph_id is provided via command line
        if let Some(ref id) = self.graph_id {
            let graph_id = GraphId::from_str(id)?;
            return Ok(graph_id);
        }

        let suggested_id = generate_graph_id(project_name, &mut DefaultRandomStringGenerator, None);

        // Enter prompt/validate loop
        self.prompt_graph_id(suggested_id.into_string())
    }

    fn prompt_graph_id(&self, suggested_id: String) -> RoverResult<GraphId> {
        let mut attempt = 1;

        loop {
            let input = self.prompt_for_input(&suggested_id)?;

            // If the user just pressed Enter (empty input), use the suggested ID
            let input_to_validate = if input.is_empty() {
                &suggested_id
            } else {
                &input
            };

            match GraphId::from_str(input_to_validate) {
                Ok(graph_id) => {
                    return Ok(graph_id);
                }
                Err(e) => {
                    let _ = self.handle_validation_error(e, attempt);
                }
            }

            attempt += 1;
        }
    }

    fn prompt_for_input(&self, suggested_id: &str) -> RoverResult<String> {
        let input = Input::<String>::new()
            .with_prompt(Style::Prompt.paint("? Confirm or modify graph ID (start with a letter and use only letters, numbers, and dashes)"))
            .with_initial_text(suggested_id.to_string())
            .allow_empty(true)
            .interact_text()?;

        if input.is_empty() {
            Ok(suggested_id.to_string())
        } else {
            Ok(input)
        }
    }

    fn handle_validation_error(
        &self,
        error: GraphIdValidationError,
        attempt: usize,
    ) -> RoverResult<()> {
        let rover_error = validation_error_to_rover_error(error);

        eprintln!("{rover_error}");

        eprintln!("Please try again (attempt {attempt})");
        Ok(())
    }
}
