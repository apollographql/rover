use apollo_federation_types::config::RouterVersion;
use async_trait::async_trait;
use camino::{Utf8Path, Utf8PathBuf};
use semver::Version;

use crate::{
    command::{install::Plugin, Install},
    options::LicenseAccepter,
    utils::{client::StudioClientConfig, effect::install::InstallBinary},
};

use super::binary::RouterBinary;

#[derive(thiserror::Error, Debug)]
#[error("Failed to install the router")]
pub enum InstallRouterError {
    #[error("unable to find dependency: \"{err}\"")]
    MissingDependency {
        /// The error while attempting to find the dependency
        err: String,
    },
    #[error("Missing filename for path: {path}")]
    MissingFilename { path: Utf8PathBuf },
    #[error("Invalid semver version: \"{input}\"")]
    Semver {
        input: String,
        source: semver::Error,
    },
}

pub struct InstallRouter {
    studio_client_config: StudioClientConfig,
    router_version: RouterVersion,
}

impl InstallRouter {
    #[allow(unused)]
    pub fn new(
        router_version: RouterVersion,
        studio_client_config: StudioClientConfig,
    ) -> InstallRouter {
        InstallRouter {
            router_version,
            studio_client_config,
        }
    }
}

#[async_trait]
impl InstallBinary for InstallRouter {
    type Binary = RouterBinary;
    type Error = InstallRouterError;
    async fn install(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool,
    ) -> Result<Self::Binary, Self::Error> {
        let plugin = Plugin::Router(self.router_version.clone());
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
            .map_err(|err| InstallRouterError::MissingDependency {
                err: err.to_string(),
            })?;
        let version = version_from_path(&exe)?;
        let binary = RouterBinary::new(exe, version);
        Ok(binary)
    }
}

fn version_from_path(path: &Utf8Path) -> Result<Version, InstallRouterError> {
    let file_name = path
        .file_name()
        .ok_or_else(|| InstallRouterError::MissingFilename {
            path: path.to_path_buf(),
        })?;
    let without_exe = file_name.strip_suffix(".exe").unwrap_or(file_name);
    let without_prefix = without_exe.strip_prefix("router-v").unwrap_or(without_exe);
    let version = Version::parse(without_prefix).map_err(|err| InstallRouterError::Semver {
        input: without_prefix.to_string(),
        source: err,
    })?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, time::Duration};

    use anyhow::Result;
    use apollo_federation_types::config::RouterVersion;
    use assert_fs::{NamedTempFile, TempDir};
    use camino::Utf8PathBuf;
    use flate2::{write::GzEncoder, Compression};
    use houston::Config;
    use http::Method;
    use httpmock::MockServer;
    use rstest::{fixture, rstest};
    use semver::Version;
    use speculoos::prelude::*;
    use tracing_test::traced_test;

    use crate::{
        options::LicenseAccepter,
        utils::{
            client::{ClientBuilder, StudioClientConfig},
            effect::install::InstallBinary,
        },
    };

    use super::InstallRouter;

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
    fn router_version() -> RouterVersion {
        RouterVersion::Latest
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
        router_version: RouterVersion,
        studio_client_config: StudioClientConfig,
        http_server: &MockServer,
        mock_server_endpoint: &str,
    ) -> Result<()> {
        let license_accepter = LicenseAccepter {
            elv2_license_accepted: Some(true),
        };
        let override_install_path = NamedTempFile::new("override_path")?;
        let install_router = InstallRouter::new(router_version, studio_client_config);
        http_server.mock(|when, then| {
            when.matches(|request| {
                request.method == Method::HEAD.to_string()
                    && request.path.starts_with("/tar/router")
            });
            then.status(302).header("X-Version", "v1.57.1");
        });
        http_server.mock(|when, then| {
            when.matches(|request| {
                request.method == Method::GET.to_string()
                    && request.path.starts_with("/tar/router/")
            });
            then.status(302)
                .header("Location", format!("{}/router/", mock_server_endpoint));
        });

        let enc = GzEncoder::new(Vec::new(), Compression::default());
        let mut archive = tar::Builder::new(enc);
        let contents = b"router";
        let mut header = tar::Header::new_gnu();
        header.set_path("dist/router")?;
        header.set_size(contents.len().try_into().unwrap());
        header.set_cksum();
        archive.append(&header, &contents[..]).unwrap();

        let finished_archive = archive.into_inner()?;
        let finished_archive_bytes = finished_archive.finish()?;

        http_server.mock(|when, then| {
            when.matches(|request| {
                request.method == Method::GET.to_string() && request.path.starts_with("/router")
            });
            then.status(200)
                .header("Content-Type", "application/octet-stream")
                .body(&finished_archive_bytes);
        });
        let binary = temp_env::async_with_vars(
            [("APOLLO_ROVER_DOWNLOAD_HOST", Some(mock_server_endpoint))],
            async {
                install_router
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
        assert_that!(subject.version()).is_equal_to(&Version::from_str("1.57.1")?);

        let installed_binary_path = override_install_path
            .path()
            .join(".rover/bin/router-v1.57.1");
        assert_that!(subject.exe())
            .is_equal_to(&Utf8PathBuf::from_path_buf(installed_binary_path.clone()).unwrap());
        assert_that!(installed_binary_path.exists()).is_equal_to(true);
        let installed_binary_contents = std::fs::read(installed_binary_path)?;
        assert_that!(installed_binary_contents).is_equal_to(b"router".to_vec());
        Ok(())
    }
}
