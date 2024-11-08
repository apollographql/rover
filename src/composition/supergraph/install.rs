use apollo_federation_types::config::FederationVersion;
use async_trait::async_trait;
use camino::Utf8PathBuf;

use crate::{
    command::{install::Plugin, Install},
    options::LicenseAccepter,
    utils::{client::StudioClientConfig, effect::install::InstallBinary},
};

use super::{
    binary::SupergraphBinary,
    version::{SupergraphVersion, SupergraphVersionError},
};

#[derive(thiserror::Error, Debug)]
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

/// The installer for the supergraph binary. It implements InstallSupergraphBinary and has an
/// `install()` method for the actual installation. Use the installed binary path when building the
/// SupergraphBinary struct
pub struct InstallSupergraph {
    federation_version: FederationVersion,
    studio_client_config: StudioClientConfig,
}

impl InstallSupergraph {
    pub fn new(
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

#[cfg(test)]
mod tests {
    use std::{str::FromStr, time::Duration};

    use anyhow::Result;
    use apollo_federation_types::config::FederationVersion;
    use assert_fs::{NamedTempFile, TempDir};
    use camino::Utf8PathBuf;
    use flate2::{write::GzEncoder, Compression};
    use houston::Config;
    use httpmock::{Method, MockServer};
    use rstest::{fixture, rstest};
    use semver::Version;
    use speculoos::prelude::*;
    use tracing_test::traced_test;

    use crate::{
        composition::supergraph::version::SupergraphVersion,
        options::LicenseAccepter,
        utils::{
            client::{ClientBuilder, StudioClientConfig},
            effect::install::InstallBinary,
        },
    };

    use super::InstallSupergraph;

    #[fixture]
    #[once]
    fn http_server() -> MockServer {
        MockServer::start()
    }

    #[fixture]
    #[once]
    fn mock_server_endpoint(http_server: &MockServer) -> String {
        let address = http_server.address();
        let endpoint = format!("http://{}", address);
        endpoint
    }

    #[fixture]
    #[once]
    fn home() -> TempDir {
        TempDir::new().unwrap()
    }

    #[fixture]
    fn federation_version() -> FederationVersion {
        FederationVersion::LatestFedTwo
    }

    #[fixture]
    #[once]
    fn api_key() -> String {
        "api-key".to_string()
    }

    #[fixture]
    fn config(api_key: &str, home: &TempDir) -> Config {
        let home = Utf8PathBuf::from_path_buf(home.to_path_buf()).unwrap();
        Config {
            home,
            override_api_key: Some(api_key.to_string()),
        }
    }

    #[fixture]
    fn studio_client_config(mock_server_endpoint: &str, config: Config) -> StudioClientConfig {
        StudioClientConfig::new(
            Some(mock_server_endpoint.to_string()),
            config,
            false,
            ClientBuilder::default(),
            None,
        )
    }

    #[traced_test]
    #[tokio::test]
    #[rstest]
    #[timeout(Duration::from_secs(15))]
    async fn test_install(
        federation_version: FederationVersion,
        studio_client_config: StudioClientConfig,
        http_server: &MockServer,
        mock_server_endpoint: &str,
    ) -> Result<()> {
        let license_accepter = LicenseAccepter {
            elv2_license_accepted: Some(true),
        };
        let override_install_path = NamedTempFile::new("override_path")?;
        let install_supergraph = InstallSupergraph::new(federation_version, studio_client_config);
        http_server.mock(|when, then| {
            when.matches(|request| {
                request.method == Method::HEAD.to_string()
                    && request.path.starts_with("/tar/supergraph")
            });
            then.status(302).header("X-Version", "v2.9.0");
        });
        http_server.mock(|when, then| {
            when.matches(|request| {
                request.method == Method::GET.to_string()
                    && request.path.starts_with("/tar/supergraph/")
            });
            then.status(302)
                .header("Location", format!("{}/supergraph/", mock_server_endpoint));
        });

        let enc = GzEncoder::new(Vec::new(), Compression::default());
        let mut archive = tar::Builder::new(enc);
        let contents = b"supergraph";
        let mut header = tar::Header::new_gnu();
        header.set_path("dist/supergraph")?;
        header.set_size(contents.len().try_into().unwrap());
        header.set_cksum();
        archive.append(&header, &contents[..]).unwrap();

        let finished_archive = archive.into_inner()?;
        let finished_archive_bytes = finished_archive.finish()?;

        http_server.mock(|when, then| {
            when.matches(|request| {
                request.method == Method::GET.to_string() && request.path.starts_with("/supergraph")
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

        let installed_binary_path = override_install_path
            .path()
            .join(".rover/bin/supergraph-v2.9.0");
        assert_that!(subject.exe())
            .is_equal_to(&Utf8PathBuf::from_path_buf(installed_binary_path.clone()).unwrap());
        assert_that!(installed_binary_path.exists()).is_equal_to(true);
        let installed_binary_contents = std::fs::read(installed_binary_path)?;
        assert_that!(installed_binary_contents).is_equal_to(b"supergraph".to_vec());
        Ok(())
    }
}
