use anyhow::anyhow;
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
        // if --allow-invalid-routing-url is not provided, we need to inspect
        // the URL and possibly prompt the user to publish
        if !self.allow_invalid_routing_url {
            Self::handle_maybe_invalid_routing_url(
                self.routing_url.clone(),
                Self::prompt_for_publish,
            )?;
        }

        let client = client_config.get_authenticated_client(&self.profile)?;

        if self.routing_url.is_none() {
            let fetch_response = routing_url::run(
                SubgraphRoutingUrlInput {
                    graph_ref: self.graph.graph_ref.clone(),
                    subgraph_name: self.subgraph.subgraph_name.clone(),
                },
                &client,
            )?;

            Self::handle_maybe_invalid_routing_url(fetch_response, Self::prompt_for_publish)?;
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

    fn handle_maybe_invalid_routing_url(
        maybe_invalid_routing_url: Option<String>,
        prompt_fn: impl FnOnce(String) -> RoverResult<Option<bool>>,
    ) -> RoverResult<()> {
        // if a --routing-url is provided AND the URL is unparsable,
        // we need to warn and prompt the user, else we can assume a publish
        if let Some(routing_url) = maybe_invalid_routing_url {
            match Url::parse(&routing_url) {
                Ok(parsed_url) => {
                    tracing::debug!("Parsed URL: {}", parsed_url.to_string());
                    if !vec!["http", "https"].contains(&parsed_url.scheme()) {
                        Self::warn_on_invalid_routing_url(prompt_fn(format!(
                            "The `{}` protocol is not supported by router, it expects either `http` or `https`. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?", &parsed_url.scheme()
                        ))?)?;
                    } else if let Some(host) = parsed_url.host_str() {
                        if vec!["localhost", "127.0.0.1"].contains(&host) {
                            Self::warn_on_invalid_routing_url(prompt_fn(format!(
                                "The host `{}` is not routable via the public internet. Continuing the publish will make this subgraph reachable in local development only. Would you still like to publish?", host
                            ))?)?;
                        }
                    }
                }
                Err(parse_error) => {
                    tracing::debug!("Parse error: {}", parse_error.to_string());
                    Self::warn_on_invalid_routing_url(prompt_fn(format!(
                        "`{}` is not a valid routing URL. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?", routing_url
                    ))?)?;
                }
            }
        }

        Ok(())
    }

    #[cfg(test)]
    fn test_warn_on_invalid_routing_url(
        maybe_prompt_response: Option<bool>,
        stderr_patch: &mut Vec<u8>,
    ) -> RoverResult<()> {
        Self::exec_warn_on_invalid_routing_url(maybe_prompt_response, stderr_patch)
    }

    fn warn_on_invalid_routing_url(maybe_prompt_response: Option<bool>) -> RoverResult<()> {
        Self::exec_warn_on_invalid_routing_url(maybe_prompt_response, &mut std::io::stderr())
    }

    pub fn exec_warn_on_invalid_routing_url(
        // a None value here means we're not in a tty, so we can't prompt
        maybe_prompt_response: Option<bool>,
        // for testing purposes, so we can inject an output stream
        output: &mut dyn std::io::Write,
    ) -> RoverResult<()> {
        if let Some(prompt_response) = maybe_prompt_response {
            if prompt_response {
                Ok(())
            } else {
                Err(anyhow!("Publish cancelled by user").into())
            }
        } else {
            // if we're not in a tty, we didn't prompt. let's print a warning
            // but publish anyway.
            writeln!(
                output,
                "{} In a future major version of Rover, the `--allow-invalid-routing-url` flag will be required to publish a subgraph with an invalid routing URL in CI.",
                Style::WarningPrefix.paint("WARN:")
            )?;
            writeln!(
                output,
                "{} Found an invalid URL, but we can't prompt in a non-interactive environment. Publishing anyway.",
                Style::WarningPrefix.paint("WARN:")
            )?;
            Ok(())
        }
    }

    pub fn prompt_for_publish(prompt: String) -> RoverResult<Option<bool>> {
        if !atty::is(atty::Stream::Stderr) || !atty::is(atty::Stream::Stdin) {
            Ok(None)
        } else {
            match prompt_confirm_default_no(prompt.as_str()) {
                Ok(response) => Ok(Some(response)),
                Err(err) => Err(anyhow!(err).into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{command::subgraph::Publish, RoverResult};

    #[test]
    fn test_handle_invalid_routing_url_tty_confirm() {
        // In TTY, user confirms publish (no warning)
        let mut output: Vec<u8> = Vec::new();
        assert!(Publish::test_warn_on_invalid_routing_url(Some(true), &mut output).is_ok());
        assert!(String::from_utf8(output).unwrap().is_empty());
    }

    #[test]
    fn test_handle_invalid_routing_url_tty_cancelled() {
        // In TTY, user cancels publish
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::test_warn_on_invalid_routing_url(Some(false), &mut output);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Publish cancelled"));
    }

    #[test]
    fn test_handle_invalid_routing_url_no_tty() {
        // No TTY, publish anyway
        // TODO(rover v2): this behavior should change - no publish unless the
        // flag is set
        let mut output: Vec<u8> = Vec::new();
        assert!(Publish::test_warn_on_invalid_routing_url(None, &mut output).is_ok());
        let output_string = String::from_utf8(output).unwrap();
        assert!(output_string.contains("WARN:"));
        assert!(output_string.contains("In a future major version of Rover, the `--allow-invalid-routing-url` flag will be required"));
        assert!(output_string.contains(
            "Found an invalid URL, but we can't prompt in a non-interactive environment"
        ));
    }

    #[test]
    fn test_handle_invalide_url_no_tty() {
        let mut prompt_value = String::new();
        // pretending we're in a non-TTY
        let prompt_fn_non_tty = |prompt: String| -> RoverResult<Option<bool>> {
            prompt_value = prompt.clone();
            Ok(None)
        };

        assert!(Publish::handle_maybe_invalid_routing_url(
            Some("invalid-url".to_string()),
            prompt_fn_non_tty,
        )
        .is_ok());
        assert_eq!(
            prompt_value,
            "`invalid-url` is not a valid routing URL. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?"
        );
    }

    #[test]
    fn test_handle_invalid_url_tty_confirm() {
        let mut prompt_value = String::new();
        let prompt_fn_confirm = |prompt: String| -> RoverResult<Option<bool>> {
            prompt_value = prompt.clone();
            // pretending we're in a TTY and responded "yes"
            Ok(Some(true))
        };
        // let prompt_fn_deny = |prompt: String| -> RoverResult<Option<bool>> {
        //     prompt_value = prompt.clone();
        //     // pretending we're in a TTY and responded "no"
        //     Ok(Some(false))
        // };

        assert!(Publish::handle_maybe_invalid_routing_url(
            Some("invalid-url".to_string()),
            prompt_fn_confirm,
        )
        .is_ok());
        assert_eq!(
            prompt_value,
            "The URL `invalid-url` is not a valid routing URL. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?"
        );
    }

    #[test]
    fn test_handle_invalid_url_tty_deny() {
        let mut prompt_value = String::new();
        let prompt_fn_deny = |prompt: String| -> RoverResult<Option<bool>> {
            prompt_value = prompt.clone();
            // pretending we're in a TTY and responded "no"
            Ok(Some(false))
        };

        assert!(Publish::handle_maybe_invalid_routing_url(
            Some("invalid-url".to_string()),
            prompt_fn_deny,
        )
        .is_err());
        assert_eq!(
            prompt_value,
            "`invalid-url` is not a valid routing URL. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?"
        );
    }

    #[test]
    fn test_handle_invalid_url_with_localhost() {
        let mut prompt_value = String::new();
        let prompt_fn_deny = |prompt: String| -> RoverResult<Option<bool>> {
            prompt_value = prompt.clone();
            // pretending we're in a TTY and responded "no"
            Ok(Some(false))
        };

        assert!(Publish::handle_maybe_invalid_routing_url(
            Some("http://localhost:3000".to_string()),
            prompt_fn_deny,
        )
        .is_err());

        assert_eq!(
            prompt_value,
            "The host `localhost` is not routable via the public internet. Continuing the publish will make this subgraph reachable in local development only. Would you still like to publish?"
        );
    }

    #[test]
    fn test_handle_invalid_url_with_invalid_scheme() {
        let mut prompt_value = String::new();
        let prompt_fn_deny = |prompt: String| -> RoverResult<Option<bool>> {
            prompt_value = prompt.clone();
            // pretending we're in a TTY and responded "no"
            Ok(Some(false))
        };

        assert!(Publish::handle_maybe_invalid_routing_url(
            Some("ftp://abc.com".to_string()),
            prompt_fn_deny,
        )
        .is_err());

        assert_eq!(
            prompt_value,
            "The protocol `ftp` is not supported by router. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?"
        );
    }
}
