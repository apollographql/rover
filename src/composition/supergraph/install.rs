use apollo_federation_types::config::FederationVersion;
use async_trait::async_trait;
use camino::Utf8PathBuf;

use super::{
    binary::SupergraphBinary,
    version::{SupergraphVersion, SupergraphVersionError},
};
use crate::{
    command::{Install, install::Plugin},
    options::LicenseAccepter,
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
    #[error(transparent)]
    SupergraphVersion(#[from] SupergraphVersionError),
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

        let exe = install_command
            .get_versioned_plugin(
                override_install_path,
                self.studio_client_config.clone(),
                skip_update,
            )
            .await
            .map_err(|err| InstallSupergraphError::MissingDependency {
                err: err.to_string(),
            })?;

        let version = SupergraphVersion::try_from(&exe)?;
        let binary = SupergraphBinary::builder()
            .exe(exe)
            .version(version)
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
        composition::supergraph::version::SupergraphVersion,
        options::LicenseAccepter,
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
}
