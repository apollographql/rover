use crate::command::install::Plugin;
use crate::command::Install;
use crate::federation::supergraph_config::ResolvedSupergraphConfig;
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
// TODO: nice constructor & channels instead of pub fields
#[derive(Debug)]
pub(crate) struct Composer {
    pub(crate) supergraph_config: ResolvedSupergraphConfig,
    supergraph_binary_path: Utf8PathBuf,
}

impl Composer {
    /// Create a new composer using `initial_config` for the first composition, and then watching
    /// `supergraph_yaml_path` for changes.
    pub(crate) async fn new(
        initial_config: ResolvedSupergraphConfig,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool, // TODO: encapsulate this with override_install_path?
    ) -> RoverResult<Self> {
        let supergraph_binary_path = Self::maybe_install_supergraph(
            override_install_path,
            client_config,
            initial_config.federation_version.clone(),
            elv2_license_accepter,
            skip_update,
        )
        .await?;
        Ok(Self {
            supergraph_config: initial_config,
            supergraph_binary_path,
        })
    }

    pub(crate) async fn compose(
        &self,
        output_file: Option<Utf8PathBuf>,
    ) -> RoverResult<BuildResult> {
        let mut output_file = output_file;

        // We mutate the supergraph_config before sending it to the supergraph binary
        let mut supergraph_config = self.supergraph_config.clone();
        // Overwrite the federation_version with _only_ the major version
        // before sending it to the supergraph plugin.
        // we do this because the supergraph binaries _only_ check if the major version is correct
        // and we may want to introduce other semver things in the future.
        // this technique gives us forward _and_ backward compatibility
        // because the supergraph plugin itself only has to parse "federation_version: 1" or "federation_version: 2"
        supergraph_config.federation_version = match self.supergraph_config.federation_version.get_major_version() {
            0 | 1 => FederationVersion::LatestFedOne,
            2 => FederationVersion::LatestFedTwo,
            _ => unreachable!("This version of Rover does not support major versions of federation other than 1 and 2.")
        };
        let supergraph_config_yaml = serde_yaml::to_string(&supergraph_config)?;
        let dir = tempfile::Builder::new().prefix("supergraph").tempdir()?;
        tracing::debug!("temp dir created at {}", dir.path().display());
        let yaml_path = Utf8PathBuf::try_from(dir.path().join("config.yml"))?;
        let mut f = File::create(&yaml_path)?;
        f.write_all(supergraph_config_yaml.as_bytes())?;
        f.sync_all()?;
        tracing::debug!("config file written to {}", &yaml_path);

        // TODO: we should be getting this version when we download and setting it in `self.supergraph_config`
        let federation_version = Self::extract_federation_version(&self.supergraph_binary_path)?;
        let exact_version = federation_version
            .get_exact()
            // This should be impossible to get to because we convert to a FederationVersion a few
            // lines above and so _should_ have an exact version
            .ok_or(RoverError::new(anyhow!(
                "failed to get exact Federation version"
            )))?;

        eprintln!(
            "composing supergraph with Federation {}",
            &federation_version.get_tarball_version()
        );

        // When the `--output` flag is used, we need a supergraph binary version that is at least
        // v2.9.0. We ignore that flag for composition when we have anything less than that
        if output_file.is_some()
            && (exact_version.major < 2 || (exact_version.major == 2 && exact_version.minor < 9))
        {
            warnln!("ignoring `--output` because it is not supported in this version of the dependent binary, `supergraph`: {}. Upgrade to Federation 2.9.0 or greater to install a version of the binary that supports it.", federation_version);
            output_file = None;
        }

        // Whether we use stdout or a file dependson whether the the `--output` option was used
        let content = match output_file {
            // If it was, we use a file in the supergraph binary; this cuts down the overall time
            // it takes to do composition when we're working on really large compositions, but it
            // carries with it the assumption that stdout is superfluous
            Some(filepath) => {
                Command::new(&self.supergraph_binary_path)
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
                let output = Command::new(&self.supergraph_binary_path)
                    .args(["compose", yaml_path.as_ref()])
                    .output()
                    .context("Failed to execute command")?;

                let content = from_utf8(&output.stdout).with_context(|| {
                    format!(
                        "Could not parse output of `{} compose`",
                        self.supergraph_binary_path
                    )
                })?;
                content.to_string()
            }
        };

        // Make sure the composition is well-formed
        serde_json::from_str::<BuildResult>(&content).map_err(|err| {
            let err = anyhow!("{}", err)
                .context(format!(
                    "{} compose output: {}",
                    self.supergraph_binary_path, content
                ))
                .context(format!(
                    "Output from `{} compose` was malformed.",
                    self.supergraph_binary_path
                ));
            let mut error = RoverError::new(err);
            error.set_suggestion(RoverErrorSuggestion::SubmitIssue);
            error
        })
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
    fn extract_federation_version(exe: &Utf8PathBuf) -> Result<FederationVersion, RoverError> {
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

        match version.major {
            0 | 1 => Ok(FederationVersion::ExactFedOne(version)),
            2 => Ok(FederationVersion::ExactFedTwo(version)),
            _ => Err(RoverError::new(anyhow!("unsupported Federation version"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::federation::format_version;
    use rstest::rstest;
    use speculoos::assert_that;

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
        let result = Composer::extract_federation_version(&fake_path).unwrap();
        assert_that(&result).matches(|f| format_version(f) == expected_value);
    }
}
