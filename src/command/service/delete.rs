use crate::client::get_rover_client;
use anyhow::Result;
use rover_client::query::service::delete::{self, DeleteServiceResponse};
use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Delete {
    /// The variant of the request graph from Apollo Studio
    #[structopt(long, default_value = "current")]
    #[serde(skip_serializing)]
    variant: String,

    /// The unique graph name that this schema is being pushed to
    #[structopt(long)]
    #[serde(skip_serializing)]
    graph_name: String,

    /// Name of the configuration profile (default: "default")
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,

    /// Name of the implementing service in the graph to update with a new schema
    #[structopt(long)]
    #[serde(skip_serializing)]
    service_name: String,
}

impl Delete {
    pub fn run(&self) -> Result<()> {
        let client = get_rover_client(&self.profile_name)?;

        tracing::info!(
            "Deleting service {} from graph {}@{}, mx. {}!",
            &self.service_name,
            &self.graph_name,
            &self.variant,
            &self.profile_name
        );

        let delete_response = delete::run(
            delete::delete_service_mutation::Variables {
                id: self.graph_name.clone(),
                graph_variant: self.variant.clone(),
                name: self.service_name.clone(),
            },
            client,
        )?;

        handle_response(
            delete_response,
            &self.service_name,
            &self.graph_name,
            &self.variant,
        );
        Ok(())
    }
}

fn handle_response(
    response: DeleteServiceResponse,
    service_name: &str,
    graph: &str,
    variant: &str,
) {
    if response.updated_gateway {
        tracing::info!(
            "The {} service was removed from {}@{}. Remaining services were composed.",
            service_name,
            graph,
            variant
        )
    } else {
        tracing::error!(
            "The gateway for graph {} was not updated. Check errors below.",
            graph
        )
    }

    if let Some(errors) = response.composition_errors {
        tracing::error!(
            "There were composition errors as a result of deleting the service: \n{}",
            errors.join("\n")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{handle_response, DeleteServiceResponse};

    #[test]
    fn handle_response_doesnt_error_with_all_successes() {
        let response = DeleteServiceResponse {
            composition_errors: None,
            updated_gateway: true,
        };

        handle_response(response, "accounts", "my-graph", "current");
    }

    #[test]
    fn handle_response_doesnt_error_with_all_failures() {
        let response = DeleteServiceResponse {
            composition_errors: Some(vec![
                "a bad thing happened".to_string(),
                "another bad thing".to_string(),
            ]),
            updated_gateway: false,
        };

        handle_response(response, "accounts", "my-graph", "prod");
    }

    // TODO: test the actual output of the logs whenever we do design work
    // for the commands :)
}
