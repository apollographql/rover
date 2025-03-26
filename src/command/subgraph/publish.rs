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

use crate::command::subgraph::publish_shared::{determine_routing_url, fetch_routing_url};
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

        let url = determine_routing_url(
            self.no_url,
            &self.routing_url,
            self.allow_invalid_routing_url,
            fetch_routing_url(&self.graph.graph_ref, &self.subgraph.subgraph_name, &client),
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
}

#[cfg(test)]
mod tests {
    use crate::command::subgraph::publish_shared::{
        determine_routing_url, handle_maybe_invalid_routing_url,
    };

    #[tokio::test]
    async fn test_no_url() {
        let mut input: &[u8] = &[];
        let mut output: Vec<u8> = Vec::new();
        let result = determine_routing_url(
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
        let result = determine_routing_url(
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
        let result = determine_routing_url(
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
        let result = determine_routing_url(
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
        let result = determine_routing_url(
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

        let result = determine_routing_url(
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

        let result = determine_routing_url(
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
        let result = handle_maybe_invalid_routing_url(
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
        let result = handle_maybe_invalid_routing_url(
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
        let result = handle_maybe_invalid_routing_url(
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
        let result = handle_maybe_invalid_routing_url(
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
        let result = handle_maybe_invalid_routing_url(
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
        let result = handle_maybe_invalid_routing_url(
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
