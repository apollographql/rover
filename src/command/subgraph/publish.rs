use clap::Parser;
use rover_std::prompt::prompt_confirm_default_no;
use serde::Serialize;
use url::Url;

use crate::options::{GraphRefOpt, ProfileOpt, SchemaOpt, SubgraphOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

use rover_client::operations::subgraph::fetch::{self, SubgraphFetchInput};
use rover_client::operations::subgraph::publish::{self, SubgraphPublishInput};

use rover_client::shared::GitContext;
use rover_std::Style;

#[derive(Debug, Serialize, Parser)]
pub struct Publish {
    #[clap(flatten)]
    graph: GraphRefOpt,

    #[clap(flatten)]
    subgraph: SubgraphOpt,

    #[clap(flatten)]
    profile: ProfileOpt,

    #[clap(flatten)]
    #[serde(skip_serializing)]
    schema: SchemaOpt,

    /// Indicate whether to convert a non-federated graph into a subgraph
    #[arg(short, long)]
    convert: bool,

    /// Url of a running subgraph that a supergraph can route operations to
    /// (often a deployed subgraph). May be left empty ("") or a placeholder url
    /// if not running a gateway or router in managed federation mode
    #[arg(long)]
    routing_url: Option<String>,
    /// Url of a running subgraph that a supergraph can route operations to
    /// (often a deployed subgraph). May be left empty ("") or a placeholder url
    /// if not running a gateway or router in managed federation mode
    #[arg(long, requires("routing-url"))]
    allow_invalid_routing_url: bool,
}

impl Publish {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> RoverResult<RoverOutput> {
        // if a --routing-url is provided AND unparsable AND
        // --allow-invalid-routing-url is not provided, we need to make some
        // decisions, otherwise we can assume a publish
        if let Some(routing_url) = &self.routing_url {
            if Url::parse(routing_url).is_err() && !self.allow_invalid_routing_url {
                if let Some(result) = Self::warn_maybe_prompt(atty::is(atty::Stream::Stdout)) {
                    return Ok(result);
                }
            }
        }

        // below is borrowed heavily from the `Fetch` command
        let client = client_config.get_authenticated_client(&self.profile)?;

        if self.routing_url.is_none() {
            let fetch_response = fetch::run(
                SubgraphFetchInput {
                    graph_ref: self.graph.graph_ref.clone(),
                    subgraph_name: self.subgraph.subgraph_name.clone(),
                },
                &client,
            )?;

            if let rover_client::shared::SdlType::Subgraph {
                routing_url: Some(graph_registry_routing_url),
            } = fetch_response.sdl.r#type
            {
                if Url::parse(&graph_registry_routing_url).is_err() {
                    // if a `Some(RoverOutput)` is returned, the user cancelled
                    // the publish and we can return here.
                    if let Some(result) = Self::warn_maybe_prompt(atty::is(atty::Stream::Stdout)) {
                        return Ok(result);
                    }
                }
            }
        }

        eprintln!(
            "Publishing SDL to {} (subgraph: {}) using credentials from the {} profile.",
            Style::Link.paint(&self.graph.graph_ref.to_string()),
            Style::Link.paint(&self.subgraph.subgraph_name),
            Style::Command.paint(&self.profile.profile_name)
        );

        let schema = self
            .schema
            .read_file_descriptor("SDL", &mut std::io::stdin())?;

        tracing::debug!("Publishing \n{}", &schema);

        let publish_response = publish::run(
            SubgraphPublishInput {
                graph_ref: self.graph.graph_ref.clone(),
                subgraph: self.subgraph.subgraph_name.clone(),
                url: self.routing_url.clone(),
                schema,
                git_context,
                convert_to_federated_graph: self.convert,
            },
            &client,
        )?;

        Ok(RoverOutput::SubgraphPublishResponse {
            graph_ref: self.graph.graph_ref.clone(),
            subgraph: self.subgraph.subgraph_name.clone(),
            publish_response,
        })
    }

    pub fn warn_maybe_prompt(is_atty: bool) -> Option<RoverOutput> {
        // if we're in a tty, prompt the user
        if is_atty {
            match prompt_confirm_default_no(
                "Found an invalid URL, would you still like to publish? [y/N]: ",
            ) {
                Ok(response) => {
                    if response {
                        return None;
                    } else {
                        eprintln!("Publish cancelled by user");
                        return Some(RoverOutput::EmptySuccess);
                    }
                }
                Err(e) => {
                    return Some(RoverOutput::ErrorExplanation(e.to_string()));
                }
            }
        } else {
            // if we're not in a tty, we can't prompt. let's print a warning but publish anyway.
            println!(
                "{} Found an invalid URL, but we can't prompt in a non-interactive environment. Publishing anyway.",
                Style::WarningPrefix.paint("WARN:")
            );
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::command::subgraph::Publish;

    #[test]
    fn test_warn_maybe_prompt() {
        let result = Publish::warn_maybe_prompt(true);
        // FIXME: how to test when y/N input is required?

        dbg!(result);
    }
}
