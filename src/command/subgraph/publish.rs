use ansi_term::Colour::{Cyan, Red, Yellow};
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverStdout;
use crate::utils::{
    client::StudioClientConfig,
    loaders::load_schema_from_flag,
    parsers::{parse_schema_source, SchemaSource},
};
use crate::Result;

use rover_client::operations::subgraph::publish::{
    self, SubgraphPublishInput, SubgraphPublishResponse,
};
use rover_client::shared::{GitContext, GraphRef};

#[derive(Debug, Serialize, StructOpt)]
pub struct Publish {
    /// <NAME>@<VARIANT> of federated graph in Apollo Studio to publish to.
    /// @<VARIANT> may be left off, defaulting to @current
    #[structopt(name = "GRAPH_REF")]
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

    /// Indicate whether to convert a non-federated graph into a subgraph
    #[structopt(short, long)]
    convert: bool,

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
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        let graph_ref = format!("{}:{}", &self.graph.name, &self.graph.variant);
        eprintln!(
            "Publishing SDL to {} (subgraph: {}) using credentials from the {} profile.",
            Cyan.normal().paint(&graph_ref),
            Cyan.normal().paint(&self.subgraph),
            Yellow.normal().paint(&self.profile_name)
        );

        let schema = load_schema_from_flag(&self.schema, std::io::stdin())?;

        tracing::debug!("Publishing \n{}", &schema);

        let publish_response = publish::run(
            SubgraphPublishInput {
                graph_ref: self.graph.clone(),
                subgraph: self.subgraph.clone(),
                url: self.routing_url.clone(),
                schema,
                git_context,
                convert_to_federated_graph: self.convert,
            },
            &client,
        )?;

        handle_publish_response(publish_response, &self.subgraph, &self.graph.name);
        Ok(RoverStdout::None)
    }
}

fn handle_publish_response(response: SubgraphPublishResponse, subgraph: &str, graph: &str) {
    if response.subgraph_was_created {
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
        eprintln!("{} The following composition errors occurred:", warn_prefix,);
        for error in errors {
            eprintln!("{}", &error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{handle_publish_response, SubgraphPublishResponse};
    use rover_client::shared::CompositionError;

    // this test is a bit weird, since we can't test the output. We just verify it
    // doesn't error
    #[test]
    fn handle_response_doesnt_error_with_all_successes() {
        let response = SubgraphPublishResponse {
            schema_hash: Some("123456".to_string()),
            did_update_gateway: true,
            subgraph_was_created: true,
            composition_errors: None,
        };

        handle_publish_response(response, "accounts", "my-graph");
    }

    #[test]
    fn handle_response_doesnt_error_with_all_failures() {
        let response = SubgraphPublishResponse {
            schema_hash: None,
            did_update_gateway: false,
            subgraph_was_created: false,
            composition_errors: Some(vec![
                CompositionError {
                    message: "a bad thing happened".to_string(),
                    code: None,
                },
                CompositionError {
                    message: "another bad thing".to_string(),
                    code: None,
                },
            ]),
        };

        handle_publish_response(response, "accounts", "my-graph");
    }

    // TODO: test the actual output of the logs whenever we do design work
    // for the commands :)
}
