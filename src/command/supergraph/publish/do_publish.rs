use std::io::stdin;

use anyhow::anyhow;
use clap::{Args, Parser};
use derive_getters::Getters;
use rover_client::operations::supergraph::publish::{self, SupergraphPublishInput};
use rover_client::shared::{GitContext, GraphRef};
use rover_std::Style;
use serde::Serialize;
use tower::ServiceExt;

use crate::options::{GraphRefOpt, PluginOpts};
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::FileDescriptorType;
use crate::{RoverError, RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Publish {
    #[clap(flatten)]
    opts: SupergraphPublishOpts,

    #[clap(flatten)]
    graph_ref: GraphRefOpt,
}

#[cfg_attr(test, derive(Default))]
#[derive(Clone, Args, Debug, Serialize, Getters)]
#[group(required = true)]
pub struct SupergraphConfigSource {
    /// The relative path to the supergraph configuration file. You can pass `-` to use stdin instead of a file.
    #[serde(skip_serializing)]
    #[arg(long = "config")]
    supergraph_yaml: Option<FileDescriptorType>,
}

#[cfg_attr(test, derive(Default))]
#[derive(Clone, Debug, Serialize, Parser, Getters)]
pub struct SupergraphPublishOpts {
    #[clap(flatten)]
    pub plugin_opts: PluginOpts,

    #[clap(flatten)]
    pub supergraph_config_source: SupergraphConfigSource,
}

impl Publish {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
    ) -> RoverResult<RoverOutput> {
        use crate::composition::{
            pipeline::CompositionPipeline,
            supergraph::config::{
                full::introspect::MakeResolveIntrospectSubgraph,
                resolver::{
                    fetch_remote_subgraph::MakeFetchRemoteSubgraph,
                    fetch_remote_subgraphs::MakeFetchRemoteSubgraphs,
                },
            },
        };

        let supergraph_yaml = self
            .opts
            .clone()
            .supergraph_config_source()
            .clone()
            .supergraph_yaml;

        let profile = self.opts.plugin_opts.profile.clone();
        let graph_ref = self.graph_ref.graph_ref.clone();

        let fetch_remote_subgraphs_factory = MakeFetchRemoteSubgraphs::builder()
            .studio_client_config(client_config.clone())
            .profile(profile.clone())
            .build();

        let fetch_remote_subgraph_factory = MakeFetchRemoteSubgraph::builder()
            .studio_client_config(client_config.clone())
            .profile(profile.clone())
            .build()
            .boxed_clone();

        let resolve_introspect_subgraph_factory =
            MakeResolveIntrospectSubgraph::new(client_config.service()?).boxed_clone();

        // We initialize the `CompositionPipeline` with the provided supergraph configuration.
        // This is a bit of a misnomer in this context as we are not actually composing a supergraph, but rather using the existing configuration to resolve subgraphs (including SDLs and routing URLs) for publishing.
        let composition_state = CompositionPipeline::default()
            .init(
                &mut stdin(),
                fetch_remote_subgraphs_factory.clone(),
                supergraph_yaml.clone(),
                None,
                None,
            )
            .await?
            .state;

        // We'll then resolve the subgraphs from the configuration using `fully_resolve_subgraphs`.
        let resolved_subgraphs = composition_state
            .resolver
            .fully_resolve_subgraphs(
                resolve_introspect_subgraph_factory,
                fetch_remote_subgraph_factory,
                &composition_state.supergraph_root,
            )
            .await?;

        let publish_subgraph_names = resolved_subgraphs
            .0
            .subgraphs
            .iter()
            .map(|s| s.1.name().to_string())
            .collect::<Vec<String>>();

        if publish_subgraph_names.is_empty() {
            return Err(RoverError::new(anyhow!(
                "No subgraphs found to publish. Please check your supergraph configuration"
            )));
        }

        let input = SupergraphPublishInput {
            graph_ref: GraphRef {
                name: graph_ref.clone().name,
                variant: graph_ref.clone().variant,
            },
            // We collect the subgraph inputs from the resolved subgraphs, converting them into the format expected by the publish operation.
            subgraph_inputs: resolved_subgraphs
                .0
                .subgraphs
                .into_iter()
                .map(|s| {
                    let subgraph = s.1;
                    publish::SupergraphPublishSubgraphInput {
                        subgraph: subgraph.name().to_string(),
                        url: subgraph.routing_url.clone(),
                        schema: subgraph.schema().to_string(),
                    }
                })
                .collect(),
            git_context,
        };

        // We log the graph reference and the subgraph names being published to the console so users know what is happening.
        eprintln!(
            "Publishing subgraph(s) {} to {}",
            Style::Link.paint(graph_ref.clone().to_string()),
            Style::Command.paint(publish_subgraph_names.clone().join(", "))
        );

        // We'll get an authenticated client from the `StudioClientConfig` and then run the publish operation with the input we constructed.
        let client = client_config.get_authenticated_client(&self.opts.plugin_opts.profile)?;

        // Run the publish operation with the constructed input and the authenticated client.
        let publish_response = publish::run(input, &client).await?;

        Ok(RoverOutput::SupergraphPublishResponse {
            graph_ref: graph_ref.clone(),
            publishing_subgraphs: publish_subgraph_names.clone(),
            publish_response,
        })
    }
}
