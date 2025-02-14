use std::io::stdin;

use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use clap::{Args, Parser};
use derive_getters::Getters;
use rover_client::shared::GraphRef;
use serde::Serialize;
use tower::ServiceExt;

use crate::options::PluginOpts;
use crate::utils::client::StudioClientConfig;
use crate::utils::effect::exec::TokioCommand;
use crate::utils::effect::write_file::{FsWriteFile, WriteFile};
use crate::utils::parsers::FileDescriptorType;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
pub struct Compose {
    #[clap(flatten)]
    opts: SupergraphComposeOpts,
}

#[cfg_attr(test, derive(Default))]
#[derive(Clone, Args, Debug, Serialize, Getters)]
#[group(required = true)]
pub struct SupergraphConfigSource {
    /// The relative path to the supergraph configuration file. You can pass `-` to use stdin instead of a file.
    #[serde(skip_serializing)]
    #[arg(long = "config")]
    supergraph_yaml: Option<FileDescriptorType>,

    /// A [`GraphRef`] that is accessible in Apollo Studio.
    /// This is used to initialize your supergraph with the values contained in this variant.
    ///
    /// This is analogous to providing a supergraph.yaml file with references to your graph variant in studio.
    ///
    /// If used in conjunction with `--config`, the values presented in the supergraph.yaml will take precedence over these values.
    #[arg(long = "graph-ref")]
    graph_ref: Option<GraphRef>,
}

#[cfg_attr(test, derive(Default))]
#[derive(Clone, Debug, Serialize, Parser, Getters)]
pub struct SupergraphComposeOpts {
    #[clap(flatten)]
    pub plugin_opts: PluginOpts,

    #[clap(flatten)]
    pub supergraph_config_source: SupergraphConfigSource,

    /// The version of Apollo Federation to use for composition. If no version is supplied, Rover
    /// will automatically determine the version from the supergraph config
    #[arg(long = "federation-version")]
    pub federation_version: Option<FederationVersion>,
}

impl Compose {
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        output_file: Option<Utf8PathBuf>,
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

        let write_file_impl = FsWriteFile::default();
        let exec_command_impl = TokioCommand::default();
        let supergraph_yaml = self
            .opts
            .clone()
            .supergraph_config_source()
            .clone()
            .supergraph_yaml;

        let profile = self.opts.plugin_opts.profile.clone();
        let graph_ref = self.opts.supergraph_config_source.graph_ref.clone();

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

        let composition_pipeline = CompositionPipeline::default()
            .init(
                &mut stdin(),
                fetch_remote_subgraphs_factory,
                supergraph_yaml,
                graph_ref.clone(),
                None,
            )
            .await?
            .resolve_federation_version(
                resolve_introspect_subgraph_factory,
                fetch_remote_subgraph_factory,
                self.opts.federation_version.clone(),
            )
            .await
            .install_supergraph_binary(
                client_config.clone(),
                override_install_path.clone(),
                self.opts.plugin_opts.elv2_license_accepter,
                self.opts.plugin_opts.skip_update,
            )
            .await?;
        let composition_success = composition_pipeline
            .compose(&exec_command_impl, &write_file_impl)
            .await?;

        if let Some(output_file) = output_file {
            write_file_impl
                .write_file(&output_file, composition_success.supergraph_sdl.as_bytes())
                .await?;
        }

        Ok(RoverOutput::CompositionResult(composition_success.into()))
    }
}
