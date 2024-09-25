use crate::command::install::Plugin;
use crate::command::Install;
use crate::federation::supergraph_config::{ResolvedSubgraphConfig, ResolvedSupergraphConfig};
use crate::options::LicenseAccepter;
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverErrorSuggestion, RoverResult};
use anyhow::{anyhow, Context};
use apollo_federation_types::config::{FederationVersion, PluginVersion};
use apollo_federation_types::rover::BuildResult;
use camino::Utf8PathBuf;
use rover_std::warnln;
use semver::Version;
use std::fs::File;
use std::io::{Read, Write};
use std::process::Command;
use std::str::from_utf8;

/// Takes the configuration for composing a supergraph and composes it. Also can watch that file and
/// all subgraphs for changes, recomposing and emitting events when they occur.
#[derive(Clone, Debug)]
pub(crate) struct Composer {
    supergraph_config: ResolvedSupergraphConfig,
    binary: SupergraphBinary,
}

impl Composer {
    /// Create a new composer using `initial_config` for the first composition, and then watching
    /// `supergraph_yaml_path` for changes.
    pub(crate) async fn new(
        mut initial_config: ResolvedSupergraphConfig,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool, // TODO: encapsulate this with override_install_path?
    ) -> RoverResult<Self> {
        let binary = SupergraphBinary::new(
            override_install_path,
            client_config,
            initial_config.federation_version.clone(),
            elv2_license_accepter,
            skip_update,
        )
        .await?;

        // Overwrite the federation_version with _only_ the major version
        // we do this because the supergraph binaries _only_ check if the major version is correct
        // and we may want to introduce other semver things in the future.
        // this technique gives us forward _and_ backward compatibility
        // because the supergraph plugin itself only has to parse "federation_version: 1" or "federation_version: 2"
        initial_config.federation_version = match initial_config.federation_version.get_major_version() {
            0 | 1 => FederationVersion::LatestFedOne,
            2 => FederationVersion::LatestFedTwo,
            _ => unreachable!("This version of Rover does not support major versions of federation other than 1 and 2.")
        };
        Ok(Self {
            supergraph_config: initial_config,
            binary,
        })
    }

    pub(crate) async fn compose(
        &self,
        output_file: Option<Utf8PathBuf>,
    ) -> RoverResult<BuildResult> {
        let mut output_file = output_file;

        let supergraph_config_yaml = serde_yaml::to_string(&self.supergraph_config)?;
        let dir = tempfile::Builder::new().prefix("supergraph").tempdir()?;
        tracing::debug!("temp dir created at {}", dir.path().display());
        let yaml_path = Utf8PathBuf::try_from(dir.path().join("config.yml"))?;
        let mut f = File::create(&yaml_path)?;
        f.write_all(supergraph_config_yaml.as_bytes())?;
        f.sync_all()?;
        tracing::debug!("config file written to {}", &yaml_path);

        let federation_version = &self.binary.federation_version;
        let exact_version = &self.binary.exact_federation_version;

        // When the `--output` flag is used, we need a supergraph binary version that is at least
        // v2.9.0. We ignore that flag for composition when we have anything less than that
        if output_file.is_some()
            && (exact_version.major < 2 || (exact_version.major == 2 && exact_version.minor < 9))
        {
            warnln!("ignoring `--output` because it is not supported in this version of the dependent binary, `supergraph`: {}. Upgrade to Federation 2.9.0 or greater to install a version of the binary that supports it.", federation_version);
            output_file = None;
        }

        // Whether we use stdout or a file depends on whether the `--output` option was used
        let content = match output_file {
            // If it was, we use a file in the supergraph binary; this cuts down the overall time
            // it takes to do composition when we're working on really large compositions, but it
            // carries with it the assumption that stdout is superfluous
            Some(filepath) => {
                Command::new(&self.binary.path)
                    .args(["compose", yaml_path.as_ref(), filepath.as_ref()])
                    .output()
                    .context("Failed to execute command")?;

                let mut composition_file = File::open(&filepath)?;
                let mut content: String = String::new();
                composition_file.read_to_string(&mut content)?;
                content
            }
            // When we aren't using `--output`, we dump the composition directly to stdout
            None => {
                let output = Command::new(&self.binary.path)
                    .args(["compose", yaml_path.as_ref()])
                    .output()
                    .context("Failed to execute command")?;

                let content = from_utf8(&output.stdout).with_context(|| {
                    format!("Could not parse output of `{} compose`", self.binary.path)
                })?;
                content.to_string()
            }
        };

        // Make sure the composition is well-formed
        serde_json::from_str::<BuildResult>(&content).map_err(|err| {
            let err = anyhow!("{}", err)
                .context(format!("{} compose output: {}", self.binary.path, content))
                .context(format!(
                    "Output from `{} compose` was malformed.",
                    self.binary.path
                ));
            let mut error = RoverError::new(err);
            error.set_suggestion(RoverErrorSuggestion::SubmitIssue);
            error
        })
    }

    pub(crate) fn get_federation_version(&self) -> FederationVersion {
        self.binary.federation_version.clone()
    }

    pub(crate) async fn set_federation_version(
        mut self,
        federation_version: FederationVersion,
    ) -> RoverResult<Self> {
        self.binary = self.binary.update(federation_version).await?;
        Ok(self)
    }

    /// Set the SDL for a subgraph, return `None` if the subgraph doesn't exist.
    pub(crate) fn update_subgraph_sdl(&mut self, name: &str, new_sdl: String) -> Option<()> {
        let subgraph = self.supergraph_config.subgraphs.get_mut(name)?;
        subgraph.schema.sdl = new_sdl;
        Some(())
    }

    /// Inserts the subgraph into the underlying map, returning the old version if it existed.
    pub(crate) fn insert_subgraph(
        &mut self,
        name: String,
        new_config: ResolvedSubgraphConfig,
    ) -> Option<ResolvedSubgraphConfig> {
        self.supergraph_config.subgraphs.insert(name, new_config)
    }

    pub(crate) fn remove_subgraph(&mut self, name: &str) -> Option<ResolvedSubgraphConfig> {
        self.supergraph_config.subgraphs.remove(name)
    }
}

#[derive(Clone, Debug)]
struct SupergraphBinary {
    path: Utf8PathBuf,
    override_install_path: Option<Utf8PathBuf>,
    client_config: StudioClientConfig,
    federation_version: FederationVersion,
    exact_federation_version: Version,
    elv2_license_accepter: LicenseAccepter,
    skip_update: bool,
}

impl SupergraphBinary {
    async fn new(
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        federation_version: FederationVersion,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool,
    ) -> RoverResult<Self> {
        let path = Self::maybe_install_supergraph(
            override_install_path.clone(),
            client_config.clone(),
            federation_version.clone(),
            elv2_license_accepter,
            skip_update,
        )
        .await?;
        let exact_federation_version = Self::extract_federation_version(&path)?;
        Ok(Self {
            path,
            override_install_path,
            client_config,
            federation_version,
            elv2_license_accepter,
            skip_update,
            exact_federation_version,
        })
    }

    async fn update(self, federation_version: FederationVersion) -> RoverResult<Self> {
        Self::new(
            self.override_install_path,
            self.client_config,
            federation_version,
            self.elv2_license_accepter,
            self.skip_update,
        )
        .await
    }
    async fn maybe_install_supergraph(
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        federation_version: FederationVersion,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool,
    ) -> RoverResult<Utf8PathBuf> {
        if federation_version.is_fed_two() {
            // TODO: this should happen in `get_versioned_plugin`
            elv2_license_accepter.require_elv2_license(&client_config)?;
        }
        let plugin = Plugin::Supergraph(federation_version);

        // and create our plugin that we may need to install from it
        let install_command = Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepter,
        };

        // maybe do the install, maybe find a pre-existing installation, maybe fail
        let plugin_exe = install_command
            .get_versioned_plugin(override_install_path, client_config, skip_update)
            .await?;
        Ok(plugin_exe)
    }

    /// Extracts the Federation Version from the executable
    fn extract_federation_version(exe: &Utf8PathBuf) -> Result<Version, RoverError> {
        let file_name = exe.file_name().unwrap();
        let without_exe = file_name.strip_suffix(".exe").unwrap_or(file_name);
        let version = match Version::parse(
            without_exe
                .strip_prefix("supergraph-v")
                .unwrap_or(without_exe),
        ) {
            Ok(version) => version,
            Err(err) => return Err(RoverError::new(err)),
        };

        let federation_version = match version.major {
            0 | 1 => FederationVersion::ExactFedOne(version),
            2 => FederationVersion::ExactFedTwo(version),
            _ => return Err(RoverError::new(anyhow!("unsupported Federation version"))),
        };

        federation_version
            .get_exact()
            // This should be impossible to get to because we convert to a FederationVersion a few
            // lines above and so _should_ have an exact version
            .ok_or(RoverError::new(anyhow!(
                "failed to get exact Federation version"
            )))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use speculoos::assert_that;

    #[rstest]
    #[case::simple_binary("a/b/c/d/supergraph-v2.8.5", "2.8.5")]
    #[case::simple_windows_binary("a/b/supergraph-v2.9.1.exe", "2.9.1")]
    #[case::complicated_semver(
        "a/b/supergraph-v1.2.3-SNAPSHOT.123+asdf",
        "1.2.3-SNAPSHOT.123+asdf"
    )]
    #[case::complicated_semver_windows(
        "a/b/supergraph-v1.2.3-SNAPSHOT.123+asdf.exe",
        "1.2.3-SNAPSHOT.123+asdf"
    )]
    fn it_can_extract_a_version_correctly(#[case] file_path: &str, #[case] expected_value: &str) {
        let mut fake_path = Utf8PathBuf::new();
        fake_path.push(file_path);
        let result = SupergraphBinary::extract_federation_version(&fake_path).unwrap();
        assert_that(&result).matches(|f| f.to_string() == expected_value);
    }
}
