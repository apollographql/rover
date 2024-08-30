use std::{fs::File, io::Write, process::Command, str};

use anyhow::{anyhow, Context};
use apollo_federation_types::config::FederationVersion::LatestFedTwo;
use apollo_federation_types::config::SupergraphConfig;
use apollo_federation_types::{
    build::BuildResult,
    config::{FederationVersion, PluginVersion},
};
use camino::Utf8PathBuf;
use clap::{Args, Parser};
use serde::Serialize;

use rover_client::shared::GraphRef;
use rover_client::RoverClientError;

use crate::utils::supergraph_config::get_supergraph_config;
use crate::utils::{client::StudioClientConfig, parsers::FileDescriptorType};
use crate::{
    command::{
        install::{Install, Plugin},
        supergraph::compose::CompositionOutput,
    },
    options::PluginOpts,
    RoverError, RoverErrorSuggestion, RoverOutput, RoverResult,
};

#[derive(Debug, Serialize, Parser)]
pub struct Compose {
    #[clap(flatten)]
    opts: SupergraphComposeOpts,
}

#[derive(Args, Debug, Serialize)]
#[group(required = true)]
pub struct SupergraphConfigSource {
    /// The relative path to the supergraph configuration file. You can pass `-` to use stdin instead of a file.
    #[serde(skip_serializing)]
    #[arg(long = "config", conflicts_with = "graph_ref")]
    supergraph_yaml: Option<FileDescriptorType>,

    /// A [`GraphRef`] that is accessible in Apollo Studio.
    /// This is used to initialize your supergraph with the values contained in this variant.
    ///
    /// This is analogous to providing a supergraph.yaml file with references to your graph variant in studio.
    ///
    /// If used in conjunction with `--config`, the values presented in the supergraph.yaml will take precedence over these values.
    #[arg(long = "graph-ref", conflicts_with = "supergraph_yaml")]
    graph_ref: Option<GraphRef>,
}

#[derive(Debug, Serialize, Parser)]
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
    pub fn new(compose_opts: PluginOpts) -> Self {
        Self {
            opts: SupergraphComposeOpts {
                plugin_opts: compose_opts,
                federation_version: Some(LatestFedTwo),
                supergraph_config_source: SupergraphConfigSource {
                    supergraph_yaml: Some(FileDescriptorType::File("RAM".into())),
                    graph_ref: None,
                },
            },
        }
    }

    pub(crate) async fn maybe_install_supergraph(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        federation_version: FederationVersion,
    ) -> RoverResult<Utf8PathBuf> {
        let plugin = Plugin::Supergraph(federation_version.clone());
        if federation_version.is_fed_two() {
            self.opts
                .plugin_opts
                .elv2_license_accepter
                .require_elv2_license(&client_config)?;
        }

        // and create our plugin that we may need to install from it
        let install_command = Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepter: self.opts.plugin_opts.elv2_license_accepter,
        };

        // maybe do the install, maybe find a pre-existing installation, maybe fail
        let plugin_exe = install_command
            .get_versioned_plugin(
                override_install_path,
                client_config,
                self.opts.plugin_opts.skip_update,
            )
            .await?;
        Ok(plugin_exe)
    }

    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        let mut supergraph_config = get_supergraph_config(
            &self.opts.supergraph_config_source.graph_ref,
            &self.opts.supergraph_config_source.supergraph_yaml.clone(),
            self.opts.federation_version.as_ref(),
            client_config.clone(),
            &self.opts.plugin_opts.profile,
            true,
        )
        .await?
        // WARNING: remove this unwrap
        .unwrap();

        self.compose(override_install_path, client_config, &mut supergraph_config)
            .await
    }

    pub async fn compose(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        supergraph_config: &mut SupergraphConfig,
    ) -> RoverResult<RoverOutput> {
        let output = self
            .exec(override_install_path, client_config, supergraph_config)
            .await?;
        Ok(RoverOutput::CompositionResult(output))
    }

    pub async fn exec(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        supergraph_config: &mut SupergraphConfig,
    ) -> RoverResult<CompositionOutput> {
        // first, grab the _actual_ federation version from the config we just resolved
        // (this will always be `Some` as long as we have created with `resolve_supergraph_yaml` so it is safe to unwrap)
        let federation_version = supergraph_config.get_federation_version().unwrap();
        let exe = self
            .maybe_install_supergraph(
                override_install_path,
                client_config,
                federation_version.clone(),
            )
            .await?;

        // _then_, overwrite the federation_version with _only_ the major version
        // before sending it to the supergraph plugin.
        // we do this because the supergraph binaries _only_ check if the major version is correct
        // and we may want to introduce other semver things in the future.
        // this technique gives us forward _and_ backward compatibility
        // because the supergraph plugin itself only has to parse "federation_version: 1" or "federation_version: 2"
        let v = match federation_version.get_major_version() {
            0 | 1 => FederationVersion::LatestFedOne,
            2 => FederationVersion::LatestFedTwo,
            _ => unreachable!("This version of Rover does not support major versions of federation other than 1 and 2.")
        };
        supergraph_config.set_federation_version(v);
        let num_subgraphs = supergraph_config.get_subgraph_definitions()?.len();
        let supergraph_config_yaml = serde_yaml::to_string(&supergraph_config)?;
        let dir = tempfile::Builder::new().prefix("supergraph").tempdir()?;
        tracing::debug!("temp dir created at {}", dir.path().display());
        let yaml_path = Utf8PathBuf::try_from(dir.path().join("config.yml"))?;
        let mut f = File::create(&yaml_path)?;
        f.write_all(supergraph_config_yaml.as_bytes())?;
        f.sync_all()?;
        tracing::debug!("config file written to {}", &yaml_path);

        let federation_version = Self::extract_federation_version(&exe);

        eprintln!(
            "composing supergraph with Federation {}",
            &federation_version
        );

        let output = Command::new(&exe)
            .args(["compose", yaml_path.as_ref()])
            .output()
            .context("Failed to execute command")?;
        let stdout = str::from_utf8(&output.stdout)
            .with_context(|| format!("Could not parse output of `{} compose`", &exe))?;

        match serde_json::from_str::<BuildResult>(stdout) {
            Ok(build_result) => match build_result {
                Ok(build_output) => Ok(CompositionOutput {
                    hints: build_output.hints,
                    supergraph_sdl: build_output.supergraph_sdl,
                    federation_version: Some(federation_version.to_string()),
                }),
                Err(build_errors) => Err(RoverError::from(RoverClientError::BuildErrors {
                    source: build_errors,
                    num_subgraphs,
                })),
            },
            Err(bad_json) => Err(anyhow!("{}", bad_json))
                .with_context(|| anyhow!("{} compose output: {}", &exe, stdout))
                .with_context(|| anyhow!("Output from `{} compose` was malformed.", &exe))
                .map_err(|e| {
                    let mut error = RoverError::new(e);
                    error.set_suggestion(RoverErrorSuggestion::SubmitIssue);
                    error
                }),
        }
    }

    fn extract_federation_version(exe: &Utf8PathBuf) -> &str {
        let file_name = exe.file_name().unwrap();
        let without_exe = file_name.strip_suffix(".exe").unwrap_or(file_name);
        without_exe
            .strip_prefix("supergraph-")
            .unwrap_or(without_exe)
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::assert_that;

    use super::*;

    #[rstest]
    #[case::simple_binary("a/b/c/d/supergraph-v2.8.5", "v2.8.5")]
    #[case::simple_windows_binary("a/b/supergraph-v2.9.1.exe", "v2.9.1")]
    #[case::complicated_semver(
        "a/b/supergraph-v1.2.3-SNAPSHOT.123+asdf",
        "v1.2.3-SNAPSHOT.123+asdf"
    )]
    #[case::complicated_semver_windows(
        "a/b/supergraph-v1.2.3-SNAPSHOT.123+asdf.exe",
        "v1.2.3-SNAPSHOT.123+asdf"
    )]
    fn it_can_extract_a_version_correctly(#[case] file_path: &str, #[case] expected_value: &str) {
        let mut fake_path = Utf8PathBuf::new();
        fake_path.push(file_path);
        let result = Compose::extract_federation_version(&fake_path);
        assert_that(&result).is_equal_to(expected_value);
    }
}
