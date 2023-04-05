use clap::Parser;
use serde::Serialize;

use crate::options::{GraphRefOpt, ProfileOpt, SchemaOpt, SubgraphOpt};
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};

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
    #[arg(long, requires("routing_url"))]
    allow_invalid_routing_url: bool,
}

impl Publish {
    pub fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> RoverResult<RoverOutput> {
        self.impl_run(
            client_config,
            git_context,
            &mut std::io::stderr(),
            &mut std::io::stdin(),
        )
    }

    fn impl_run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
        stderr: &mut impl std::io::Write,
        stdin: &mut impl std::io::Read,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(&self.profile)?;
        writeln!(
            stderr,
            "Publishing SDL to {} (subgraph: {}) using credentials from the {} profile.",
            Style::Link.paint(&self.graph.graph_ref.to_string()),
            Style::Link.paint(&self.subgraph.subgraph_name),
            Style::Command.paint(&self.profile.profile_name)
        )?;

        let schema = self.schema.read_file_descriptor("SDL", stdin)?;

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
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use houston::Config;
    use rover_client::shared::{GitContext, GraphRef};

    use crate::{
        command::subgraph::Publish,
        options::{GraphRefOpt, ProfileOpt, SchemaOpt, SubgraphOpt},
        utils::client::{ClientBuilder, StudioClientConfig},
    };

    #[test]
    fn test_basic_publish() {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        let client_config = StudioClientConfig::new(
            None,
            Config::new(Some(&tmp_path), None).unwrap(),
            false,
            ClientBuilder::default(),
        );
        let git_context = GitContext::default();
        let publish = Publish {
            graph: GraphRefOpt {
                graph_ref: GraphRef::from_str("test@current").unwrap(),
            },
            subgraph: SubgraphOpt {
                subgraph_name: "subgraph".to_string(),
            },
            profile: ProfileOpt {
                profile_name: "default".to_string(),
            },
            schema: SchemaOpt { schema: None },
            convert: false,
            routing_url: None,
            allow_invalid_routing_url: false,
        };

        publish::impl_run(client_config, git_context, stderr, stdin);
        ()
    }
}
