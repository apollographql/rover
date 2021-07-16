use ansi_term::Colour::{Cyan, Red, Yellow};
use serde::Serialize;
use structopt::StructOpt;

use crate::command::RoverOutput;
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
    ) -> Result<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile_name)?;
        eprintln!(
            "Publishing SDL to {} (subgraph: {}) using credentials from the {} profile.",
            Cyan.normal().paint(&self.graph.to_string()),
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

        Ok(RoverOutput::SubgraphPublishResponse {
            graph_ref: self.graph.clone(),
            subgraph: self.subgraph.clone(),
            publish_response,
        })
    }
}
