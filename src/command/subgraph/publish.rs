use clap::Parser;
use rover_std::prompt::prompt_confirm_default_no;
use serde::Serialize;
use url::Url;

use crate::options::{GraphRefOpt, ProfileOpt, SchemaOpt, SubgraphOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

use rover_client::operations::subgraph::publish::{self, SubgraphPublishInput};
use rover_client::operations::subgraph::routing_url::{self, SubgraphRoutingUrlInput};

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
    #[serde(skip_serializing)]
    routing_url: Option<String>,
    /// Bypasses warnings and the prompt to confirm publish when the routing url
    /// is invalid in TTY environment. In a future major version, this flag will
    /// be required to publish in a non-TTY environment. For now it will warn
    /// and publish anyway.
    #[arg(long, requires("routing_url"))]
    allow_invalid_routing_url: bool,
}

impl Publish {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> RoverResult<RoverOutput> {
        // * if --allow-invalid-routing-url is not provided
        // * AND a --routing-url is provided
        // * AND the URL is unparsable
        // we need to warn and prompt the user, else we can assume a publish
        if !self.allow_invalid_routing_url {
            if let Some(routing_url) = &self.routing_url {
                if let Err(parse_error) = Url::parse(routing_url) {
                    tracing::debug!("Parse error: {}", parse_error.to_string());

                    if let Some(result) =
                        Self::handle_invalid_routing_url(Self::prompt_for_publish(), None)
                    {
                        return Ok(result);
                    }
                }
            }
        }

        // below is borrowed heavily from the `Fetch` command
        let client = client_config.get_authenticated_client(&self.profile)?;

        if self.routing_url.is_none() {
            let fetch_response = routing_url::run(
                SubgraphRoutingUrlInput {
                    graph_ref: self.graph.graph_ref.clone(),
                    subgraph_name: self.subgraph.subgraph_name.clone(),
                },
                &client,
            )?;

            if let Some(routing_url) = fetch_response {
                if let Err(parse_error) = Url::parse(&routing_url) {
                    tracing::debug!("Parse error: {}", parse_error.to_string());

                    if let Some(result) =
                        Self::handle_invalid_routing_url(Self::prompt_for_publish(), None)
                    {
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

    // output stream is optionally injected (defaulting to stdout) so we can test this function
    pub fn handle_invalid_routing_url(
        maybe_prompt_response: Option<bool>,
        maybe_output: Option<&mut dyn std::io::Write>,
    ) -> Option<RoverOutput> {
        let default_output = &mut std::io::stdout();
        let output = maybe_output.unwrap_or(default_output);
        if let Some(prompt_response) = maybe_prompt_response {
            if prompt_response {
                None
            } else {
                writeln!(output, "Publish cancelled by user")
                    .expect("Could not write to provided output stream");
                Some(RoverOutput::EmptySuccess)
            }
        } else {
            // if we're not in a tty, we didn't prompt. let's print a warning but
            // publish anyway.
            writeln!(
                output,
                "{} In a future major version of Rover, the `--allow-invalid-routing-url` flag will be required to publish a subgraph with an invalid routing URL in CI.",
                Style::WarningPrefix.paint("WARN:")
            ).expect("Could not write to provided output stream");
            writeln!(
                output,
                "{} Found an invalid URL, but we can't prompt in a non-interactive environment. Publishing anyway.",
                Style::WarningPrefix.paint("WARN:")
            ).expect("Could not write to provided output stream");
            None
        }
    }

    pub fn prompt_for_publish() -> Option<bool> {
        if !atty::is(atty::Stream::Stdout) {
            // q for Avery: not actually sure why I can't implicitly return
            // `None` here
            return None;
        }

        match prompt_confirm_default_no(
            "Found an invalid URL, would you still like to publish? [y/N]: ",
        ) {
            Ok(response) => Some(response),
            _ => panic!("Expected a response in TTY environment"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::command::subgraph::Publish;

    #[test]
    fn test_handle_invalid_routing_url() {
        // In TTY, user confirms publish (no warning)
        let mut output: Vec<u8> = Vec::new();
        assert!(Publish::handle_invalid_routing_url(Some(true), Some(&mut output)).is_none());
        assert!(String::from_utf8(output).unwrap().is_empty());

        // In TTY, user cancels publish
        output = Vec::new();
        assert!(Publish::handle_invalid_routing_url(Some(false), Some(&mut output)).is_some());
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("Publish cancelled"));

        // No TTY, publish anyway
        // TODO(rover v2): this behavior should change - no publish unless the
        // flag is set
        output = Vec::new();
        assert!(Publish::handle_invalid_routing_url(None, Some(&mut output)).is_none());
        dbg!(String::from_utf8(output.clone()).unwrap());
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("WARN:"));
        assert!(output.contains("In a future major version of Rover, the `--allow-invalid-routing-url` flag will be required"));
        assert!(output.contains(
            "Found an invalid URL, but we can't prompt in a non-interactive environment"
        ));
    }
}
