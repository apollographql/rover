use crate::command::init::graph_id::errors::conversions::validation_error_to_rover_error;
use crate::command::init::graph_id::generation::generate_graph_id;
use crate::command::init::graph_id::utils::random::DefaultRandomStringGenerator;
use crate::command::init::graph_id::{validation::GraphIdValidationError, GraphId};
use crate::RoverResult;
use clap::arg;
use clap::Parser;
use dialoguer::Input;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct GraphIdOpt {
    #[arg(long = "graph-id")]
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

            match GraphId::from_str(&input) {
                Ok(graph_id) => return Ok(graph_id),
                Err(e) => self.handle_validation_error(e, attempt)?,
            }

            attempt += 1;
        }
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
    ) -> RoverResult<()> {
        let rover_error = validation_error_to_rover_error(error);

        eprintln!("{}", rover_error);

        eprintln!("Please try again (attempt {})", attempt);
        Ok(())
    }
}
