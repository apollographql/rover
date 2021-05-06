use ansi_term::Colour::{Cyan, Red, Yellow};
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::utils::{
    client::StudioClientConfig,
    git::GitContext,
    loaders::load_schema_from_flag,
    parsers::{parse_graph_ref, parse_schema_source, GraphRef, SchemaSource},
};
use crate::Result;

use rover_client::query::subgraph::publish::{self, PublishPartialSchemaResponse};

#[derive(Debug, Serialize, StructOpt)]
pub struct Publish {
    /// <NAME>@<VARIANT> of federated graph in Apollo Studio to publish to.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF", parse(try_from_str = parse_graph_ref))]
    #[serde(skip_serializing)]
    graph: GraphRef,

    /// The schema file to publish
    /// Can pass `-` to use stdin instead of a file
    #[structopt(long, short = "s", parse(try_from_str = parse_schema_source))]
    #[serde(skip_serializing)]
    schema: SchemaSource,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,

    /// Name of subgraph in federated graph to update
    #[structopt(long = "name")]
    #[serde(skip_serializing)]
    subgraph: String,

    /// Url of a running subgraph that a gateway can route operations to
    /// (often a deployed subgraph). May be left empty ("") or a placeholder url
    /// if not running a gateway in managed federation mode
    #[structopt(long)]
    #[serde(skip_serializing)]
    routing_url: Option<String>,
}

impl Publish {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> Result<RoverStdout> {
        let client = client_config.get_client(&self.profile_name)?;
        let graph_ref = format!("{}:{}", &self.graph.name, &self.graph.variant);
        eprintln!(
            "Publishing SDL to {} (subgraph: {}) using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Cyan.normal().paint(&self.subgraph),
            Yellow.normal().paint(&self.profile_name)
        );

        let schema_document = load_schema_from_flag(&self.schema, std::io::stdin())?;

        tracing::debug!("Schema Document to publish:\n{}", &schema_document);

        let publish_response = publish::run(
            publish::publish_partial_schema_mutation::Variables {
                graph_id: self.graph.name.clone(),
                graph_variant: self.graph.variant.clone(),
                name: self.subgraph.clone(),
                active_partial_schema:
                    publish::publish_partial_schema_mutation::PartialSchemaInput {
                        sdl: Some(schema_document),
                        hash: None,
                    },
                revision: "".to_string(),
                url: self.routing_url.clone(),
                git_context: git_context.into(),
            },
            &client,
        )?;

        handle_response(publish_response, &self.subgraph, &self.graph.name);
        Ok(RoverStdout::None)
    }
}

fn handle_response(response: PublishPartialSchemaResponse, subgraph: &str, graph: &str) {
    if response.service_was_created {
        eprintln!(
            "A new subgraph called '{}' for the '{}' graph was created",
            subgraph, graph
        );
    } else {
        eprintln!(
            "The '{}' subgraph for the '{}' graph was updated",
            subgraph, graph
        );
    }

    if response.did_update_gateway {
        eprintln!("The gateway for the '{}' graph was updated with a new schema, composed from the updated '{}' subgraph", graph, subgraph);
    } else {
        eprintln!(
            "The gateway for the '{}' graph was NOT updated with a new schema",
            graph
        );
    }

    if let Some(errors) = response.composition_errors {
        let warn_prefix = Red.normal().paint("WARN:");
        eprintln!(
            "{} The following composition errors occurred: \n{}",
            warn_prefix,
            errors.join("\n")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{handle_response, PublishPartialSchemaResponse};

    // this test is a bit weird, since we can't test the output. We just verify it
    // doesn't error
    #[test]
    fn handle_response_doesnt_error_with_all_successes() {
        let response = PublishPartialSchemaResponse {
            schema_hash: Some("123456".to_string()),
            did_update_gateway: true,
            service_was_created: true,
            composition_errors: None,
        };

        handle_response(response, "accounts", "my-graph");
    }

    #[test]
    fn handle_response_doesnt_error_with_all_failures() {
        let response = PublishPartialSchemaResponse {
            schema_hash: None,
            did_update_gateway: false,
            service_was_created: false,
            composition_errors: Some(vec![
                "a bad thing happened".to_string(),
                "another bad thing".to_string(),
            ]),
        };

        handle_response(response, "accounts", "my-graph");
    }

    // TODO: test the actual output of the logs whenever we do design work
    // for the commands :)
}
