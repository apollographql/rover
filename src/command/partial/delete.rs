use crate::client::get_studio_client;
use crate::command::RoverStdout;
use anyhow::Result;
use rover_client::query::partial::delete::{self, DeleteServiceResponse};
use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Delete {
    /// Variant of the graph in Apollo Studio
    #[structopt(long, default_value = "current")]
    #[serde(skip_serializing)]
    variant: String,

    /// ID of the graph in Apollo Studio to delete a service from
    #[structopt(long)]
    #[serde(skip_serializing)]
    graph_name: String,

    /// Name of the configuration profile (default: "default")
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,

    /// Name of the implementing service in the graph to delete
    #[structopt(long)]
    #[serde(skip_serializing)]
    service_name: String,

    /// Skip the delete preview as well as the prompt confirming deletion
    #[structopt(long)]
    confirm: bool,
}

impl Delete {
    pub fn run(&self) -> Result<RoverStdout> {
        let client = get_studio_client(&self.profile_name)?;

        tracing::info!(
            "Checking for composition errors resulting from deleting service `{}` from graph {}@{}, mx. {}!",
            &self.service_name,
            &self.graph_name,
            &self.variant,
            &self.profile_name
        );

        // this is probably the normal path -- preview a service delete
        // and make the user confirm it manually.
        if !self.confirm {
            // run delete with dryRun, so we can preview composition errors
            let delete_dry_run_response = delete::run(
                delete::delete_service_mutation::Variables {
                    id: self.graph_name.clone(),
                    graph_variant: self.variant.clone(),
                    name: self.service_name.clone(),
                    dry_run: true,
                },
                client,
            )?;

            handle_dry_run_response(
                delete_dry_run_response,
                &self.service_name,
                &self.graph_name,
                &self.variant,
            );

            // I chose not to error here, since this is a perfectly valid path
            if !confirm_delete()? {
                tracing::info!("Delete cancelled by user");
                return Ok(RoverStdout::None);
            }
        }

        let client = get_studio_client(&self.profile_name)?;

        let delete_response = delete::run(
            delete::delete_service_mutation::Variables {
                id: self.graph_name.clone(),
                graph_variant: self.variant.clone(),
                name: self.service_name.clone(),
                dry_run: false,
            },
            client,
        )?;

        handle_response(
            delete_response,
            &self.service_name,
            &self.graph_name,
            &self.variant,
        );
        Ok(RoverStdout::None)
    }
}

fn handle_dry_run_response(
    response: DeleteServiceResponse,
    service_name: &str,
    graph: &str,
    variant: &str,
) {
    if let Some(errors) = response.composition_errors {
        tracing::warn!(
                "Deleting the {} service from {}@{} would result in the following composition errors: \n{}",
                service_name,
                graph,
                variant,
                errors.join("\n")
            );
        tracing::warn!("Note: This is only a prediction. If the graph changes before confirming, these errors could change.");
    } else {
        tracing::info!("At the time of checking, there would be no composition errors resulting from the deletion of this graph.");
        tracing::warn!("Note: This is only a prediction. If the graph changes before confirming, there could be composition errors.")
    }
}

fn confirm_delete() -> Result<bool> {
    tracing::info!("Would you like to continue [y/n]");
    let term = console::Term::stdout();
    let confirm = term.read_line()?;
    if confirm.to_lowercase() == *"y" {
        Ok(true)
    } else {
        Ok(false)
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
