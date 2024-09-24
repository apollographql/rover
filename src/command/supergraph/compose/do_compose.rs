use anyhow::anyhow;
use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use clap::{Args, Parser};
use derive_getters::Getters;
use rover_client::{shared::GraphRef, RoverClientError};
use serde::Serialize;

use crate::federation::supergraph_config::{get_supergraph_config, resolve_supergraph_config};
use crate::federation::{format_version, Composer};
use crate::{
    command::supergraph::compose::CompositionOutput,
    options::PluginOpts,
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
    RoverError, RoverOutput, RoverResult,
};

#[derive(Debug, Serialize, Parser)]
pub struct Compose {
    #[clap(flatten)]
    opts: SupergraphComposeOpts,
}

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

#[derive(Clone, Debug, Serialize, Parser, Getters)]
pub struct SupergraphComposeOpts {
    #[clap(flatten)]
    pub plugin_opts: PluginOpts,

    #[clap(flatten)]
    pub supergraph_config_source: SupergraphConfigSource,

    /// The version of Apollo Federation to use for composition
    #[arg(long = "federation-version")]
    federation_version: Option<FederationVersion>,
}

impl Compose {
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        output_file: Option<Utf8PathBuf>,
    ) -> RoverResult<RoverOutput> {
        let supergraph_config = get_supergraph_config(
            &self.opts.supergraph_config_source.graph_ref,
            self.opts.supergraph_config_source.supergraph_yaml.as_ref(),
            self.opts.federation_version.as_ref(),
            client_config.clone(),
            &self.opts.plugin_opts.profile,
        )
        .await?
        .ok_or_else(|| anyhow!("error getting supergraph config"))?;

        let resolved_supergraph_config = resolve_supergraph_config(
            supergraph_config.merged_config,
            client_config.clone(),
            &self.opts.plugin_opts.profile,
        )
        .await?;

        let composer = Composer::new(
            resolved_supergraph_config,
            override_install_path,
            client_config.clone(),
            self.opts.plugin_opts.elv2_license_accepter,
            self.opts.plugin_opts.skip_update,
        )
        .await?;

        match composer.compose(output_file).await? {
            Ok(build_output) => Ok(RoverOutput::CompositionResult(CompositionOutput {
                hints: build_output.hints,
                supergraph_sdl: build_output.supergraph_sdl,
                federation_version: Some(format_version(
                    // TODO: this should be the exact version from the binary, instead
                    &composer.supergraph_config.federation_version,
                )),
            })),
            Err(build_errors) => Err(RoverError::from(RoverClientError::BuildErrors {
                source: build_errors,
            })),
        }
    }
}
