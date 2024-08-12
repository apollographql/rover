use std::io::{self, IsTerminal};

use anyhow::anyhow;
use clap::Parser;
use futures::Future;
use reqwest::Url;
use rover_client::operations::subgraph::routing_url::{self, SubgraphRoutingUrlInput};
use serde::Serialize;

use crate::options::{GraphRefOpt, ProfileOpt, SchemaOpt, SubgraphOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverErrorSuggestion, RoverOutput, RoverResult};

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
    #[serde(skip_serializing)]
    routing_url: Option<String>,

    /// Bypasses warnings and the prompt to confirm publish when the routing url
    /// is invalid in TTY environment. In a future major version, this flag will
    /// be required to publish in a non-TTY environment. For now it will warn
    /// and publish anyway.
    #[arg(long)]
    allow_invalid_routing_url: bool,

    /// This is shorthand for `--routing-url "" --allow-invalid-routing-url`.
    #[arg(long)]
    no_url: bool,
}

impl Publish {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;

        let url = Self::determine_routing_url(
            self.no_url,
            &self.routing_url,
            self.allow_invalid_routing_url,
            || async {
                Ok(routing_url::run(
                    SubgraphRoutingUrlInput {
                        graph_ref: self.graph.graph_ref.clone(),
                        subgraph_name: self.subgraph.subgraph_name.clone(),
                    },
                    &client,
                )
                .await?)
            },
            &mut io::stderr(),
            &mut io::stdin(),
            io::stderr().is_terminal() && io::stdin().is_terminal(),
        )
        .await?;

        eprintln!(
            "Publishing SDL to {} (subgraph: {}) using credentials from the {} profile.",
            Style::Link.paint(self.graph.graph_ref.to_string()),
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
                url,
                schema,
                git_context,
                convert_to_federated_graph: self.convert,
            },
            &client,
        )
        .await?;

        Ok(RoverOutput::SubgraphPublishResponse {
            graph_ref: self.graph.graph_ref.clone(),
            subgraph: self.subgraph.subgraph_name.clone(),
            publish_response,
        })
    }

    async fn determine_routing_url<F, G>(
        no_url: bool,
        routing_url: &Option<String>,
        allow_invalid_routing_url: bool,

        // For testing purposes, we pass in a closure for fetching the
        // routing url from GraphOS
        fetch: F,
        // For testing purposes, we pass in stub `Write`er and `Read`ers to
        // simulate input and verify output.
        writer: &mut impl io::Write,
        reader: &mut impl io::Read,
        // Simulate a CI environment (non-TTY) for testing
        is_atty: bool,
    ) -> RoverResult<Option<String>>
    where
        F: Fn() -> G,
        G: Future<Output = RoverResult<String>>,
    {
        if no_url && routing_url.is_some() {
            return Err(RoverError::new(anyhow!(
                "You cannot use --no-url and --routing-url at the same time."
            )));
        }

        // if --allow-invalid-routing-url is not provided, we need to inspect
        // the URL and possibly prompt the user to publish. this does nothing
        // if the routing url is not provided.
        if !no_url && !allow_invalid_routing_url {
            Self::handle_maybe_invalid_routing_url(routing_url, writer, reader, is_atty)?;
        }

        // don't bother fetching and validating an existing routing url if
        // --no-url is set
        let mut routing_url = routing_url.clone();
        if !no_url && routing_url.is_none() {
            let fetch_response = fetch().await?;
            Self::handle_maybe_invalid_routing_url(
                &Some(fetch_response.clone()),
                writer,
                reader,
                is_atty,
            )?;
            routing_url = Some(fetch_response)
        }

        if let Some(routing_url) = routing_url {
            Ok(Some(routing_url))
        } else if no_url {
            // --no-url is shorthand for --routing-url ""
            Ok(Some("".to_string()))
        } else {
            Ok(None)
        }
    }

    fn handle_maybe_invalid_routing_url(
        maybe_invalid_routing_url: &Option<String>,
        // For testing purposes, we pass in stub `Write`er and `Read`ers to
        // simulate input and verify output.
        writer: &mut impl io::Write,
        reader: &mut impl io::Read,
        // Simulate a CI environment (non-TTY) for testing
        is_atty: bool,
    ) -> RoverResult<()> {
        // if a --routing-url is provided AND the URL is unparsable,
        // we need to warn and prompt the user, else we can assume a publish
        if let Some(routing_url) = maybe_invalid_routing_url {
            match Url::parse(routing_url) {
                Ok(parsed_url) => {
                    tracing::debug!("Parsed URL: {}", parsed_url.to_string());
                    let reason = format!("`{}` is not a valid routing URL. The `{}` protocol is not supported by the router. Valid protocols are `http` and `https`.", Style::Link.paint(routing_url), &parsed_url.scheme());
                    if !["http", "https", "unix"].contains(&parsed_url.scheme()) {
                        if is_atty {
                            Self::prompt_for_publish(
                                format!("{reason} Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?").as_str(),
                                reader,
                                writer,
                            )?;
                        } else {
                            Self::non_tty_hard_error(&reason)?;
                        }
                    } else if let Some(host) = parsed_url.host_str() {
                        if ["localhost", "127.0.0.1"].contains(&host) {
                            let reason = format!("The host `{}` is not routable via the public internet. Continuing the publish will make this subgraph reachable in local environments only.", host);
                            if is_atty {
                                Self::prompt_for_publish(
                                    format!("{reason} Would you still like to publish?").as_str(),
                                    reader,
                                    writer,
                                )?;
                            } else {
                                Self::non_tty_warn_about_local_url(&reason, writer)?;
                            }
                        }
                    }
                }
                Err(parse_error) => {
                    tracing::debug!("Parse error: {}", parse_error.to_string());
                    let reason = format!(
                        "`{}` is not a valid routing URL.",
                        Style::Link.paint(routing_url)
                    );
                    if is_atty {
                        Self::prompt_for_publish(
                        format!("{} Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?", &reason).as_str(),
                            reader,
                            writer,
                        )?;
                    } else {
                        Self::non_tty_hard_error(&reason)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn prompt_for_publish(
        message: &str,
        reader: &mut impl io::Read,
        writer: &mut impl io::Write,
    ) -> RoverResult<Option<bool>> {
        write!(writer, "{} [y/N] ", message)?;
        let mut response = [0];
        reader.read_exact(&mut response)?;
        if std::str::from_utf8(&response).unwrap().to_lowercase() == *"y" {
            Ok(Some(true))
        } else {
            Err(anyhow!("You cancelled a subgraph publish due to an invalid routing url.").into())
        }
    }

    pub fn non_tty_hard_error(reason: &str) -> RoverResult<()> {
        Err(RoverError::new(anyhow!("{reason}"))
            .with_suggestion(RoverErrorSuggestion::AllowInvalidRoutingUrlOrSpecifyValidUrl))
    }

    pub fn non_tty_warn_about_local_url(
        reason: &str,
        writer: &mut dyn io::Write,
    ) -> RoverResult<()> {
        writeln!(writer, "{} {reason}", Style::WarningPrefix.paint("WARN:"),)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::command::subgraph::publish::Publish;

    #[tokio::test]
    async fn test_no_url() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::determine_routing_url(
            true,
            &None,
            false,
            || async { Ok("".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();
        assert_eq!(result, Some("".to_string()));
    }

    #[tokio::test]
    async fn test_routing_url_provided() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::determine_routing_url(
            false,
            &Some("https://provided".to_string()),
            false,
            || async { Ok("".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();
        assert_eq!(result, Some("https://provided".to_string()));
    }

    #[tokio::test]
    async fn test_no_url_and_routing_url_provided() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::determine_routing_url(
            true,
            &Some("https://provided".to_string()),
            false,
            || async { Ok("".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap_err();
        assert_eq!(
            result.message(),
            "You cannot use --no-url and --routing-url at the same time."
        );
    }

    #[tokio::test]
    async fn test_routing_url_not_provided_already_exists() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::determine_routing_url(
            false,
            &None,
            false,
            || async { Ok("https://fromstudio".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();

        assert_eq!(result, Some("https://fromstudio".to_string()));
    }

    #[tokio::test]
    async fn test_routing_url_unix_socket() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::determine_routing_url(
            false,
            &None,
            false,
            || async { Ok("unix:///path/to/subgraph.sock".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();

        assert_eq!(result, Some("unix:///path/to/subgraph.sock".to_string()));
    }

    #[tokio::test]
    async fn test_routing_url_invalid_provided() {
        let mut input = "y".as_bytes();
        let mut output: Vec<u8> = Vec::new();

        let result = Publish::determine_routing_url(
            false,
            &Some("invalid".to_string()),
            false,
            || async { Ok("".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();

        assert_eq!(result, Some("invalid".to_string()));
        assert!(std::str::from_utf8(&output).unwrap().contains("is not a valid routing URL. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?"));
    }

    #[tokio::test]
    async fn test_not_url_invalid_from_studio() {
        let mut input = "y".as_bytes();
        let mut output: Vec<u8> = Vec::new();

        let result = Publish::determine_routing_url(
            true,
            &None,
            false,
            || async { Ok("invalid".to_string()) },
            &mut output,
            &mut input,
            true,
        )
        .await
        .unwrap();

        assert_eq!(result, Some("".to_string()));
        assert!(std::str::from_utf8(&output).unwrap().is_empty());
    }

    #[test]
    fn test_confirm_invalid_url_publish() {
        let mut input = "y".as_bytes();
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::handle_maybe_invalid_routing_url(
            &Some("invalid-url".to_string()),
            &mut output,
            &mut input,
            true,
        );

        assert!(result.is_ok());
        assert!(input.is_empty());
        assert!(std::str::from_utf8(&output).unwrap().contains("is not a valid routing URL. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?"));
    }

    #[test]
    fn test_deny_invalid_url_publish() {
        let mut input = "n".as_bytes();
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::handle_maybe_invalid_routing_url(
            &Some("invalid-url".to_string()),
            &mut output,
            &mut input,
            true,
        );

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("You cancelled a subgraph publish due to an invalid routing url."));
        assert!(input.is_empty());
        assert!(std::str::from_utf8(&output).unwrap().contains("is not a valid routing URL. Continuing the publish will make this subgraph unreachable by your supergraph. Would you still like to publish?"));
    }

    #[test]
    fn test_invalid_scheme() {
        let mut input = "y".as_bytes();
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::handle_maybe_invalid_routing_url(
            &Some("ftp://invalid-scheme".to_string()),
            &mut output,
            &mut input,
            true,
        );

        assert!(result.is_ok());
        assert!(input.is_empty());
        assert!(std::str::from_utf8(&output).unwrap().contains(
            "is not a valid routing URL. The `ftp` protocol is not supported by the router. Valid protocols are `http` and `https`."
        ));
    }

    #[test]
    fn test_localhost_tty() {
        let mut input = "y".as_bytes();
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::handle_maybe_invalid_routing_url(
            &Some("http://localhost:8000".to_string()),
            &mut output,
            &mut input,
            true,
        );

        assert!(result.is_ok());
        assert!(input.is_empty());
        assert!(std::str::from_utf8(&output).unwrap().contains(
            "The host `localhost` is not routable via the public internet. Continuing the publish will make this subgraph reachable in local environments only."
        ));
    }

    #[test]
    fn test_localhost_no_tty() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::handle_maybe_invalid_routing_url(
            &Some("http://localhost:8000".to_string()),
            &mut output,
            &mut input,
            false,
        );

        assert!(result.is_ok());
        assert!(input.is_empty());
        assert!(std::str::from_utf8(&output).unwrap().contains(
            "The host `localhost` is not routable via the public internet. Continuing the publish will make this subgraph reachable in local environments only."
        ));
    }

    #[test]
    fn test_invalid_url_no_tty() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = Publish::handle_maybe_invalid_routing_url(
            &Some("invalid-url".to_string()),
            &mut output,
            &mut input,
            false,
        );

        assert!(result.is_err());
        assert!(input.is_empty());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("is not a valid routing URL."));
    }
}
