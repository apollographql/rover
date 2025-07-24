use std::io::stdin;

use anyhow::anyhow;
use clap::{Args, Parser};
use derive_getters::Getters;
use rover_client::operations::supergraph::check::{
    self, SupergraphCheckInput, SupergraphCheckSubgraphInput,
};
use rover_std::Style;
use serde::Serialize;

use rover_client::operations::subgraph::check_workflow::{self, CheckWorkflowInput};
use rover_client::shared::{CheckConfig, GitContext};
use tower::ServiceExt;

use crate::options::{CheckConfigOpts, GraphRefOpt, PluginOpts};
use crate::utils::client::StudioClientConfig;
use crate::utils::parsers::FileDescriptorType;
use crate::{RoverError, RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Check {
    #[clap(flatten)]
    opts: SupergraphPublishOpts,

    #[clap(flatten)]
    graph_ref: GraphRefOpt,

    #[clap(flatten)]
    config: CheckConfigOpts,
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

impl Check {
    pub async fn run(
        &self,
        client_config: StudioClientConfig,
        git_context: GitContext,
        checks_timeout_seconds: u64,
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

        let checked_subgraphs: Vec<SupergraphCheckSubgraphInput> = resolved_subgraphs
            .0
            .subgraphs
            .into_iter()
            .map(|subgraph| SupergraphCheckSubgraphInput {
                name: subgraph.1.name().to_string(),
                sdl: subgraph.1.schema().to_string(),
            })
            .collect();

        if checked_subgraphs.is_empty() {
            return Err(RoverError::new(anyhow!(
                "No subgraphs found to check. Please check your supergraph configuration"
            )));
        }

        let pluralized_text = if checked_subgraphs.len() == 1 {
            ("schema".to_string(), "subgraph".to_string())
        } else {
            ("schemas".to_string(), "subgraphs".to_string())
        };

        let subgraph_names = checked_subgraphs
            .iter()
            .map(|s| s.name.clone())
            .collect::<Vec<String>>()
            .join(", ");
        eprintln!(
            "Checking the proposed {} for {} {} against {}",
            pluralized_text.0,
            pluralized_text.1,
            subgraph_names,
            Style::Link.paint(graph_ref.clone().to_string())
        );
        let client = client_config.get_authenticated_client(&self.opts.plugin_opts.profile)?;

        let workflow_res = check::run(
            SupergraphCheckInput {
                graph_ref: graph_ref.clone(),
                git_context,
                subgraphs_to_check: checked_subgraphs.clone(),
                config: CheckConfig {
                    query_count_threshold: self.config.query_count_threshold,
                    query_count_threshold_percentage: self.config.query_percentage_threshold,
                    validation_period: self.config.validation_period.clone(),
                },
            },
            &client,
        )
        .await?;
        if self.config.background {
            Ok(RoverOutput::AsyncCheckResponse(workflow_res))
        } else {
            let check_res = check_workflow::run(
                CheckWorkflowInput {
                    graph_ref: graph_ref.clone(),
                    workflow_id: workflow_res.workflow_id,
                    checks_timeout_seconds,
                },
                checked_subgraphs
                    .clone()
                    .iter()
                    .map(|s| s.name.clone())
                    .collect::<Vec<String>>()
                    .join(", "),
                &client,
            )
            .await?;

            Ok(RoverOutput::CheckWorkflowResponse(check_res))
        }
    }
}
