use std::fs;

use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use clap::{Args, Parser};
use derive_getters::Getters;
use rover_print::{print::Print, style::StyledText};
use rover_studio::types::GraphRef;
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult,
    command::supergraph::compose::output::ComposeOutput,
    composition::{CompositionError, get_supergraph_binary},
    options::PluginOpts,
    utils::{
        client::StudioClientConfig,
        effect::{
            exec::TokioCommand,
            write_file::{FsWriteFile, WriteFile},
        },
        parsers::FileDescriptorType,
    },
};

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
    pub supergraph_yaml: Option<FileDescriptorType>,

    /// A [`GraphRef`] that is accessible in Apollo Studio.
    /// This is used to initialize your supergraph with the values contained in this variant.
    ///
    /// This is analogous to providing a supergraph.yaml file with references to your graph variant in studio.
    ///
    /// If used in conjunction with `--config`, the values presented in the supergraph.yaml will take precedence over these values.
    #[arg(long = "graph-ref")]
    pub graph_ref: Option<GraphRef>,
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
        stderr: &impl Print,
    ) -> RoverResult<RoverOutput> {
        let write_file_impl = FsWriteFile::default();
        let exec_command_impl = TokioCommand::default();

        let composition_pipeline = get_supergraph_binary(
            self.opts.federation_version.clone(),
            client_config,
            override_install_path,
            self.opts.plugin_opts.clone(),
            self.opts.supergraph_config_source.supergraph_yaml().clone(),
            self.opts.supergraph_config_source.graph_ref().clone(),
            true,
        )
        .await?;
        // Reported before the binary is used rather than after, so a composition that
        // fails still says which plugin produced the errors (FR54, FR58) — "which
        // federation version rejected this" being the first question asked of one. It
        // also means a slow composition names its binary while you wait.
        //
        // `Err` is reachable here: resolution itself can have failed, and the run
        // then ends in an error either way — this one, or whatever `compose` hits
        // first on its way back to it. Reporting no plugin is accurate in both
        // cases rather than a gap, since nothing resolved.
        let mut plugins = Vec::new();
        if let Ok(binary) = &composition_pipeline.state.supergraph_binary {
            stderr.print(&StyledText::plain(binary.provenance().to_string()));
            plugins.push(binary.provenance().clone());
        }

        let composition_success = composition_pipeline
            .compose(&exec_command_impl, &write_file_impl)
            .await?;

        if let Some(output_file) = output_file {
            // Reported as a composition error carrying the plugin, not as a bare
            // io error. By here the plugin has not merely resolved — it has run
            // and produced the schema — so a failure to write that schema out is
            // the last place `data.plugins` should go quiet (FR57). It also
            // names the file, which the bare error did not.
            let write_error =
                |err: Box<dyn std::error::Error + Send + Sync>| CompositionError::WriteFile {
                    path: output_file.clone(),
                    error: err,
                    provenance: plugins.first().cloned().map(Box::new),
                };

            let parent = output_file.parent();
            if let Some(parent) = parent
                && !parent.exists()
            {
                fs::create_dir_all(parent).map_err(|err| write_error(Box::new(err)))?;
            }
            write_file_impl
                .write_file(&output_file, composition_success.supergraph_sdl.as_bytes())
                .await
                .map_err(|err| write_error(Box::new(err)))?;
        }

        Ok(RoverOutput::CliOutput(Box::new(ComposeOutput {
            composition: composition_success.into(),
            plugins,
        })))
    }
}
