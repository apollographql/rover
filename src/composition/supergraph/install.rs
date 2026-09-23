use apollo_federation_types::config::FederationVersion;
use async_trait::async_trait;
use camino::Utf8PathBuf;

use super::{binary::SupergraphBinary, version::SupergraphVersion};
use crate::{
    command::{Install, install::Plugin},
    options::LicenseAccepter,
    plugin::error::PluginFailure,
    utils::{client::StudioClientConfig, effect::install::InstallBinary},
};

#[derive(thiserror::Error, Debug, Clone)]
pub enum InstallSupergraphError {
    #[error("ELV2 license must be accepted")]
    LicenseNotAccepted,
    #[error("unable to find dependency: \"{err}\"")]
    MissingDependency {
        /// The error while attempting to find the dependency
        err: String,
    },
    /// The plugin couldn't be obtained, for a reason with its own error code.
    #[error("Couldn't obtain the `supergraph` plugin")]
    Plugin(#[source] Box<PluginFailure>),
}

/// The installer for the supergraph binary. It implements [`InstallSupergraph`] and has an
/// `install()` method for the actual installation. Use the installed binary path when building the
/// [`SupergraphBinary`] struct
pub struct InstallSupergraph {
    federation_version: FederationVersion,
    studio_client_config: StudioClientConfig,
}

impl InstallSupergraph {
    pub const fn new(
        federation_version: FederationVersion,
        studio_client_config: StudioClientConfig,
    ) -> InstallSupergraph {
        InstallSupergraph {
            federation_version,
            studio_client_config,
        }
    }
}

#[async_trait]
impl InstallBinary for InstallSupergraph {
    type Binary = SupergraphBinary;
    type Error = InstallSupergraphError;

    async fn install(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool,
    ) -> Result<Self::Binary, Self::Error> {
        if self.federation_version.is_fed_two() {
            elv2_license_accepter
                .require_elv2_license(&self.studio_client_config)
                .map_err(|_err| InstallSupergraphError::LicenseNotAccepted)?
        }

        let plugin = Plugin::Supergraph(self.federation_version.clone());

        let install_command = Install {
            force: false,
            plugin: Some(plugin),
            elv2_license_accepter,
        };

        let provenance = install_command
            .get_versioned_plugin(
                override_install_path,
                self.studio_client_config.clone(),
                skip_update,
            )
            .await
            .map_err(|err| match err.plugin_failure() {
                Some(failure) => InstallSupergraphError::Plugin(Box::new(failure.clone())),
                None => InstallSupergraphError::MissingDependency {
                    err: err.to_string(),
                },
            })?;

        let version = SupergraphVersion::new(provenance.version.clone());
        let binary = SupergraphBinary::builder()
            .exe(provenance.path.clone())
            .version(version)
            .provenance(provenance)
            .build();

        Ok(binary)
    }
}

#[cfg(not(target_env = "musl"))]
#[cfg(test)]
mod tests {
    use std::{str::FromStr, time::Duration};

    use anyhow::Result;
    use apollo_federation_types::config::FederationVersion;
    use assert_fs::{NamedTempFile, TempDir};
    use camino::Utf8PathBuf;
    use flate2::{Compression, write::GzEncoder};
    use houston::Config;
    use httpmock::{Method, MockServer};
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;
    use tracing_test::traced_test;

    use super::InstallSupergraph;
    use crate::{
        RoverError, RoverErrorCode,
        composition::{
            CompositionError, pipeline::CompositionPipelineError,
            supergraph::version::SupergraphVersion,
        },
        options::LicenseAccepter,
        plugin::{
            error::PluginFailure,
            version::{PluginName, VersionRequest},
        },
        utils::{
            client::{ClientBuilder, ClientTimeout, StudioClientConfig},
            effect::install::InstallBinary,
        },
    };

    #[traced_test]
    #[tokio::test]
    #[rstest]
    #[timeout(Duration::from_secs(15))]
    async fn test_install() -> Result<()> {
        let http_server = MockServer::start();
        let mock_server_endpoint = format!("http://{}", http_server.address());
        let studio_client_config = StudioClientConfig::new(
            Some(mock_server_endpoint.to_string()),
            Config {
                home: Utf8PathBuf::from_path_buf(TempDir::new().unwrap().to_path_buf()).unwrap(),
                override_api_key: Some("api-key".to_string()),
                override_client_credentials_token: None,
            },
            false,
            ClientBuilder::default(),
            ClientTimeout::default(),
        );
        let license_accepter = LicenseAccepter {
            elv2_license_accepted: Some(true),
        };
        let override_install_path = NamedTempFile::new("override_path")?;
        let install_supergraph =
            InstallSupergraph::new(FederationVersion::LatestFedTwo, studio_client_config);
        http_server.mock(|when, then| {
            when.is_true(|request| {
                request.method() == Method::HEAD
                    && request.uri().path().starts_with("/tar/supergraph")
            });
            then.status(302).header("X-Version", "v2.9.0");
        });
        http_server.mock(|when, then| {
            when.is_true(|request| {
                request.method() == Method::GET
                    && request.uri().path().starts_with("/tar/supergraph/")
            });
            then.status(302)
                .header("Location", format!("{mock_server_endpoint}/supergraph/"));
        });

        let enc = GzEncoder::new(Vec::new(), Compression::default());
        let mut archive = tar::Builder::new(enc);
        let contents = b"supergraph";
        let mut header = tar::Header::new_gnu();
        if cfg!(windows) {
            header.set_path("dist/supergraph.exe")?;
        } else {
            header.set_path("dist/supergraph")?;
        }
        header.set_size(contents.len().try_into().unwrap());
        header.set_cksum();
        archive.append(&header, &contents[..]).unwrap();

        let finished_archive = archive.into_inner()?;
        let finished_archive_bytes = finished_archive.finish()?;

        http_server.mock(|when, then| {
            when.is_true(|request| {
                request.method() == Method::GET && request.uri().path().starts_with("/supergraph")
            });
            then.status(200)
                .header("Content-Type", "application/octet-stream")
                .body(&finished_archive_bytes);
        });
        let binary = temp_env::async_with_vars(
            [("APOLLO_ROVER_DOWNLOAD_HOST", Some(mock_server_endpoint))],
            async {
                install_supergraph
                    .install(
                        Utf8PathBuf::from_path_buf(override_install_path.to_path_buf()).ok(),
                        license_accepter,
                        false,
                    )
                    .await
            },
        )
        .await;
        let subject = assert_that!(binary).is_ok().subject;
        assert_that!(subject.version())
            .is_equal_to(&SupergraphVersion::new(Version::from_str("2.9.0")?));

        let bin_name = if cfg!(windows) {
            "supergraph-v2.9.0.exe"
        } else {
            "supergraph-v2.9.0"
        };

        let installed_binary_path = override_install_path
            .path()
            .join(".rover/bin")
            .join(bin_name);
        assert_that!(subject.exe())
            .is_equal_to(&Utf8PathBuf::from_path_buf(installed_binary_path.clone()).unwrap());
        assert_that!(installed_binary_path.exists()).is_equal_to(true);
        let installed_binary_contents = std::fs::read(installed_binary_path)?;
        assert_that!(installed_binary_contents).is_equal_to(b"supergraph".to_vec());
        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    #[rstest]
    #[timeout(Duration::from_secs(15))]
    async fn install_falls_back_to_installed_plugin_when_registry_unreachable() -> Result<()> {
        let http_server = MockServer::start();
        let mock_server_endpoint = format!("http://{}", http_server.address());

        // The install path whose `.rover/bin` already holds a compatible plugin.
        let install_home = TempDir::new().unwrap();
        let override_install_path = Utf8PathBuf::from_path_buf(install_home.to_path_buf()).unwrap();
        let bin_name = if cfg!(windows) {
            "supergraph-v2.9.0.exe"
        } else {
            "supergraph-v2.9.0"
        };
        let bin_dir = install_home.path().join(".rover/bin");
        std::fs::create_dir_all(&bin_dir)?;
        let installed_binary_path = bin_dir.join(bin_name);
        std::fs::write(&installed_binary_path, b"supergraph")?;

        let studio_client_config = StudioClientConfig::new(
            Some(mock_server_endpoint.to_string()),
            Config {
                home: Utf8PathBuf::from_path_buf(TempDir::new().unwrap().to_path_buf()).unwrap(),
                override_api_key: Some("api-key".to_string()),
                override_client_credentials_token: None,
            },
            false,
            ClientBuilder::default(),
            // The failing registry is retried until this runs out; keep it short, since
            // this test is about what happens after resolution gives up.
            ClientTimeout::new(1),
        );
        let license_accepter = LicenseAccepter {
            elv2_license_accepted: Some(true),
        };
        let install_supergraph =
            InstallSupergraph::new(FederationVersion::LatestFedTwo, studio_client_config);

        // The registry is unreachable: resolving the latest version (a HEAD to the
        // tarball URL) fails, so the download can't proceed.
        http_server.mock(|when, then| {
            when.is_true(|request| {
                request.method() == Method::HEAD
                    && request.uri().path().starts_with("/tar/supergraph")
            });
            then.status(500);
        });

        let binary = temp_env::async_with_vars(
            [("APOLLO_ROVER_DOWNLOAD_HOST", Some(mock_server_endpoint))],
            async {
                install_supergraph
                    .install(Some(override_install_path), license_accepter, false)
                    .await
            },
        )
        .await;

        // Despite the failed download, we fall back to the already-installed plugin
        // rather than erroring.
        let subject = assert_that!(binary).is_ok().subject;
        assert_that!(subject.version())
            .is_equal_to(&SupergraphVersion::new(Version::from_str("2.9.0")?));
        assert_that!(subject.exe())
            .is_equal_to(&Utf8PathBuf::from_path_buf(installed_binary_path).unwrap());
        Ok(())
    }

    #[traced_test]
    #[tokio::test]
    #[rstest]
    #[timeout(Duration::from_secs(15))]
    async fn install_fails_when_registry_unreachable_and_no_fallback_available() -> Result<()> {
        let http_server = MockServer::start();
        let mock_server_endpoint = format!("http://{}", http_server.address());

        // The install path whose `.rover/bin` already holds a compatible plugin.
        let install_home = TempDir::new().unwrap();
        let override_install_path = Utf8PathBuf::from_path_buf(install_home.to_path_buf()).unwrap();

        let studio_client_config = StudioClientConfig::new(
            Some(mock_server_endpoint.to_string()),
            Config {
                home: Utf8PathBuf::from_path_buf(TempDir::new().unwrap().to_path_buf()).unwrap(),
                override_api_key: Some("api-key".to_string()),
                override_client_credentials_token: None,
            },
            false,
            ClientBuilder::default(),
            // The failing registry is retried until this runs out; keep it short, since
            // this test is about what happens after resolution gives up.
            ClientTimeout::new(1),
        );
        let license_accepter = LicenseAccepter {
            elv2_license_accepted: Some(true),
        };
        let install_supergraph =
            InstallSupergraph::new(FederationVersion::LatestFedTwo, studio_client_config);

        // The registry is unreachable: resolving the latest version (a HEAD to the
        // tarball URL) fails, so the download can't proceed.
        http_server.mock(|when, then| {
            when.is_true(|request| {
                request.method() == Method::HEAD
                    && request.uri().path().starts_with("/tar/supergraph")
            });
            then.status(500);
        });

        let binary = temp_env::async_with_vars(
            [("APOLLO_ROVER_DOWNLOAD_HOST", Some(mock_server_endpoint))],
            async {
                install_supergraph
                    .install(Some(override_install_path), license_accepter, false)
                    .await
            },
        )
        .await;

        assert_that!(binary).is_err();
        Ok(())
    }

    /// `APOLLO_ROVER_SKIP_UPDATE` opts out of plugin auto-updates: an on-the-fly
    /// install must use the already-installed plugin and never contact the
    /// registry, even when `skip_update` wasn't passed on the command. See #1892.
    #[traced_test]
    #[tokio::test]
    #[rstest]
    #[timeout(Duration::from_secs(15))]
    async fn skip_update_env_uses_installed_plugin_without_contacting_registry() -> Result<()> {
        let http_server = MockServer::start();
        let mock_server_endpoint = format!("http://{}", http_server.address());
        // Any request to the registry means the opt-out failed. (A failed request
        // would also trigger the fallback and still yield 2.9.0, so we assert on
        // zero calls rather than on the returned version.)
        let registry = http_server.mock(|when, then| {
            when.is_true(|request| request.uri().path().starts_with("/tar/supergraph"));
            then.status(500);
        });

        let install_home = TempDir::new().unwrap();
        let override_install_path = Utf8PathBuf::from_path_buf(install_home.to_path_buf()).unwrap();
        let bin_name = if cfg!(windows) {
            "supergraph-v2.9.0.exe"
        } else {
            "supergraph-v2.9.0"
        };
        let bin_dir = install_home.path().join(".rover/bin");
        std::fs::create_dir_all(&bin_dir)?;
        std::fs::write(bin_dir.join(bin_name), b"supergraph")?;

        let studio_client_config = StudioClientConfig::new(
            Some(mock_server_endpoint.to_string()),
            Config {
                home: Utf8PathBuf::from_path_buf(TempDir::new().unwrap().to_path_buf()).unwrap(),
                override_api_key: Some("api-key".to_string()),
                override_client_credentials_token: None,
            },
            false,
            ClientBuilder::default(),
            ClientTimeout::default(),
        );
        let license_accepter = LicenseAccepter {
            elv2_license_accepted: Some(true),
        };
        let install_supergraph =
            InstallSupergraph::new(FederationVersion::LatestFedTwo, studio_client_config);

        let binary = temp_env::async_with_vars(
            [
                ("APOLLO_ROVER_DOWNLOAD_HOST", Some(mock_server_endpoint)),
                ("APOLLO_ROVER_SKIP_UPDATE", Some("true".to_string())),
            ],
            async {
                // `skip_update` is false here; the env var is what forces the opt-out.
                install_supergraph
                    .install(Some(override_install_path), license_accepter, false)
                    .await
            },
        )
        .await;

        let subject = assert_that!(binary).is_ok().subject;
        assert_that!(subject.version())
            .is_equal_to(&SupergraphVersion::new(Version::from_str("2.9.0")?));
        // The decisive check: opting out meant the registry was never contacted.
        assert_that!(registry.calls()).is_equal_to(0);
        Ok(())
    }

    enum RegistryFault {
        /// The registry can't resolve the floating request.
        Resolution,
        /// The registry resolves the request, but won't serve the artifact.
        Download,
        /// The registry serves an artifact that isn't a gzipped tarball.
        CorruptArchive,
    }

    /// Each way the registry can fail maps onto its own error code and next
    /// step, and survives being wrapped by the on-the-fly installer.
    #[tokio::test]
    #[rstest]
    #[case::resolution(RegistryFault::Resolution)]
    #[case::download(RegistryFault::Download)]
    #[case::corrupt_archive(RegistryFault::CorruptArchive)]
    #[timeout(Duration::from_secs(15))]
    async fn a_failed_install_reports_its_failure_class(
        #[case] fault: RegistryFault,
    ) -> Result<()> {
        let http_server = MockServer::start();
        let mock_server_endpoint = format!("http://{}", http_server.address());
        let install_home = TempDir::new().unwrap();
        let override_install_path = Utf8PathBuf::from_path_buf(install_home.to_path_buf()).unwrap();
        let install_root = override_install_path.join(".rover").join("bin");

        http_server.mock(|when, then| {
            when.method(Method::HEAD).path_prefix("/tar/supergraph");
            match fault {
                RegistryFault::Resolution => then.status(404),
                _ => then.status(302).header("X-Version", "v2.9.0"),
            };
        });
        http_server.mock(|when, then| {
            when.method(Method::GET).path_prefix("/tar/supergraph");
            match fault {
                RegistryFault::CorruptArchive => then.status(200).body("not a tarball"),
                _ => then.status(404),
            };
        });

        let studio_client_config = StudioClientConfig::new(
            Some(mock_server_endpoint.to_string()),
            Config {
                home: Utf8PathBuf::from_path_buf(TempDir::new().unwrap().to_path_buf()).unwrap(),
                override_api_key: Some("api-key".to_string()),
                override_client_credentials_token: None,
            },
            false,
            ClientBuilder::default(),
            ClientTimeout::new(1),
        );
        let install_supergraph =
            InstallSupergraph::new(FederationVersion::LatestFedTwo, studio_client_config);
        let license_accepter = LicenseAccepter {
            elv2_license_accepted: Some(true),
        };

        let result = temp_env::async_with_vars(
            [
                ("APOLLO_ROVER_DOWNLOAD_HOST", Some(mock_server_endpoint)),
                // Would move the install root the corrupt-archive case names.
                ("APOLLO_NODE_MODULES_BIN_DIR", None),
            ],
            async {
                install_supergraph
                    .install(Some(override_install_path), license_accepter, false)
                    .await
            },
        )
        .await;

        let error = RoverError::new(result.expect_err("the install should fail"));
        let reported = (
            error.code(),
            error.plugin_failure().map(ToString::to_string),
            error
                .suggestions()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        );

        let expected = match fault {
            RegistryFault::Resolution => (
                Some(RoverErrorCode::E048),
                "Couldn't resolve a release of the `supergraph` plugin matching `2` from the plugin registry.".to_string(),
                "Make sure the plugin registry is reachable and that `supergraph` has a release matching `2`, then re-run the command. If you use a registry other than Apollo's, check that `APOLLO_ROVER_DOWNLOAD_HOST` points at it.".to_string(),
            ),
            RegistryFault::Download => (
                Some(RoverErrorCode::E049),
                "Couldn't download the `supergraph` plugin v2.9.0.".to_string(),
                "Re-run the command to retry downloading `supergraph` v2.9.0. If the download keeps failing or timing out, check your connection to the plugin registry, or allow it longer by passing `--client-timeout` a value above the 300-second default for plugin downloads.".to_string(),
            ),
            RegistryFault::CorruptArchive => (
                Some(RoverErrorCode::E050),
                format!("Couldn't install the `supergraph` plugin v2.9.0 into `{install_root}`."),
                format!("Make sure `{install_root}` is writable and has free space, then run `rover install --plugin supergraph@=2.9.0 --force --elv2-license accept` to reinstall it."),
            ),
        };
        assert_that!(reported).is_equal_to((expected.0, Some(expected.1), vec![expected.2]));
        Ok(())
    }

    fn resolution_failure() -> super::InstallSupergraphError {
        super::InstallSupergraphError::Plugin(Box::new(PluginFailure::Resolution {
            plugin: PluginName::Supergraph,
            requested: VersionRequest::Major(2),
            source: std::sync::Arc::new(std::io::Error::other("Connect error")),
        }))
    }

    /// The composition errors wrap the installer's; none of them may hide the
    /// plugin failure from the chain, or `compose`, `dev`, and `lsp` lose the
    /// code.
    #[rstest]
    #[case::pipeline(anyhow::Error::new(CompositionPipelineError::from(resolution_failure())))]
    #[case::binary(anyhow::Error::new(CompositionError::InstallSupergraphBinaryError {
        source: resolution_failure(),
    }))]
    #[case::federation_version_change(anyhow::Error::new(
        CompositionError::ErrorUpdatingFederationVersion(resolution_failure())
    ))]
    fn a_plugin_failure_keeps_its_code_through_the_composition_errors(
        #[case] error: anyhow::Error,
    ) {
        assert_that!(RoverError::new(error).code()).is_equal_to(Some(RoverErrorCode::E048));
    }
}
