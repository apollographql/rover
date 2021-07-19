use ansi_term::Colour::{Cyan, Red, Yellow};
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::Result;

use rover_client::operations::subgraph::delete::{
    self, SubgraphDeleteInput, SubgraphDeleteResponse,
};
use rover_client::shared::GraphRef;

#[derive(Debug, Serialize, StructOpt)]
pub struct Delete {
    /// <NAME>@<VARIANT> of federated graph in Apollo Studio to delete subgraph from.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,

    /// Name of subgraph in federated graph to delete
    #[structopt(long = "name")]
    #[serde(skip_serializing)]
    subgraph: String,

    /// Skips the step where the command asks for user confirmation before
    /// deleting the subgraph. Also skips preview of composition errors that
    /// might occur
    #[structopt(long)]
    confirm: bool,
}

impl Delete {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        let graph_ref = self.graph.to_string();
        eprintln!(
            "Checking for composition errors resulting from deleting subgraph {} from {} using credentials from the {} profile.",
            Cyan.normal().paint(&self.subgraph),
            Cyan.normal().paint(&graph_ref),
            Yellow.normal().paint(&self.profile_name)
        );

        // this is probably the normal path -- preview a subgraph delete
        // and make the user confirm it manually.
        if !self.confirm {
            // run delete with dryRun, so we can preview composition errors
            let delete_dry_run_response = delete::run(
                SubgraphDeleteInput {
                    graph_ref: self.graph.clone(),
                    subgraph: self.subgraph.clone(),
                    dry_run: true,
                },
                &client,
            )?;

            handle_dry_run_response(delete_dry_run_response, &self.subgraph, &graph_ref);

            // I chose not to error here, since this is a perfectly valid path
            if !confirm_delete()? {
                eprintln!("Delete cancelled by user");
                return Ok(RoverOutput::EmptySuccess);
            }
        }

        let delete_response = delete::run(
            SubgraphDeleteInput {
                graph_ref: self.graph.clone(),
                subgraph: self.subgraph.clone(),
                dry_run: false,
            },
            &client,
        )?;

        handle_response(delete_response, &self.subgraph, &graph_ref);
        Ok(RoverOutput::EmptySuccess)
    }
}

fn handle_dry_run_response(response: SubgraphDeleteResponse, subgraph: &str, graph_ref: &str) {
    let warn_prefix = Red.normal().paint("WARN:");
    if let Some(composition_errors) = response.composition_errors {
        eprintln!(
            "{} Deleting the {} subgraph from {} would result in the following composition errors:",
            warn_prefix,
            Cyan.normal().paint(subgraph),
            Cyan.normal().paint(graph_ref),
        );
        for error in composition_errors.composition_errors {
            eprintln!("{}", &error);
        }
        eprintln!("{} This is only a prediction. If the graph changes before confirming, these errors could change.", warn_prefix);
    } else {
        eprintln!("{} At the time of checking, there would be no composition errors resulting from the deletion of this subgraph.", warn_prefix);
        eprintln!("{} This is only a prediction. If the graph changes before confirming, there could be composition errors.", warn_prefix)
    }
}

fn confirm_delete() -> Result<bool> {
    eprintln!("Would you like to continue [y/n]");
    let term = console::Term::stdout();
    let confirm = term.read_line()?;
    if confirm.to_lowercase() == *"y" {
        Ok(true)
    } else {
        Ok(false)
    }
}

fn handle_response(response: SubgraphDeleteResponse, subgraph: &str, graph_ref: &str) {
    let warn_prefix = Red.normal().paint("WARN:");
    if response.updated_gateway {
        eprintln!(
            "The {} subgraph was removed from {}. Remaining subgraphs were composed.",
            Cyan.normal().paint(subgraph),
            Cyan.normal().paint(graph_ref),
        )
    } else {
        eprintln!(
            "{} The gateway for {} was not updated. See errors below.",
            warn_prefix,
            Cyan.normal().paint(graph_ref)
        )
    }

    if let Some(composition_errors) = response.composition_errors {
        eprintln!(
            "{} There were composition errors as a result of deleting the subgraph:",
            warn_prefix,
        );

        for error in composition_errors.composition_errors {
            eprintln!("{}", &error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{handle_response, SubgraphDeleteResponse};
    use rover_client::shared::{CompositionError, CompositionErrors};

    #[test]
    fn handle_response_doesnt_error_with_all_successes() {
        let response = SubgraphDeleteResponse {
            composition_errors: None,
            updated_gateway: true,
        };

        handle_response(response, "accounts", "my-graph@current");
    }

    #[test]
    fn handle_response_doesnt_error_with_all_failures() {
        let response = SubgraphDeleteResponse {
            composition_errors: Some(CompositionErrors {
                composition_errors: vec![
                    CompositionError {
                        message: "a bad thing happened".to_string(),
                        code: None,
                    },
                    CompositionError {
                        message: "another bad thing".to_string(),
                        code: None,
                    },
                ],
            }),
            updated_gateway: false,
        };

        handle_response(response, "accounts", "my-graph@prod");
    }

    // TODO: test the actual output of the logs whenever we do design work
    // for the commands :)
}
