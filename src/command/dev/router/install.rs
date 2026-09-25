use apollo_federation_types::config::RouterVersion;
use async_trait::async_trait;
use camino::Utf8PathBuf;

use super::binary::RouterBinary;
use crate::{
    command::{Install, install::Plugin},
    options::LicenseAccepter,
    utils::{client::StudioClientConfig, effect::install::InstallBinary},
};

#[derive(thiserror::Error, Debug)]
#[error("Failed to install the router")]
pub enum InstallRouterError {
    #[error("unable to find dependency: \"{err}\"")]
    MissingDependency {
        /// The error while attempting to find the dependency
        err: String,
    },
}

pub struct InstallRouter {
    studio_client_config: StudioClientConfig,
    router_version: RouterVersion,
}

impl InstallRouter {
    pub const fn new(
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
        let provenance = install_command
            .get_versioned_plugin(
                override_install_path,
                self.studio_client_config.clone(),
                skip_update,
            )
            .await
            .map_err(|err| InstallRouterError::MissingDependency {
                err: err.to_string(),
            })?;
        let binary = RouterBinary::new(provenance.path.clone(), provenance);
        Ok(binary)
    }
}

#[cfg(not(target_env = "musl"))]
#[cfg(test)]
mod tests {
    use std::{env, str::FromStr, time::Duration};

    use anyhow::Result;
    use apollo_federation_types::config::RouterVersion;
    use assert_fs::{NamedTempFile, TempDir};
    use camino::Utf8PathBuf;
    use flate2::{Compression, write::GzEncoder};
    use houston::Config;
    use http::Method;
    use httpmock::MockServer;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;
    use tracing_test::traced_test;

    use super::InstallRouter;
    use crate::{
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
                override_client_credentials_token: None,
            },
            false,
            ClientBuilder::default(),
            ClientTimeout::default(),
        )
        .with_download_host(mock_server_endpoint.clone());
        let license_accepter = LicenseAccepter {
            elv2_license_accepted: Some(true),
        };
        let override_install_path = NamedTempFile::new("override_path")?;
        let install_router = InstallRouter::new(RouterVersion::LatestTwo, studio_client_config);
        http_server.mock(|when, then| {
            when.is_true(|request| {
                request.method() == Method::HEAD && request.uri().path().starts_with("/tar/router")
            });
            then.status(302).header("X-Version", "v1.57.1");
        });
        http_server.mock(|when, then| {
            when.is_true(|request| {
                request.method() == Method::GET && request.uri().path().starts_with("/tar/router/")
            });
            then.status(302)
                .header("Location", format!("{mock_server_endpoint}/router/"));
        });

        let enc = GzEncoder::new(Vec::new(), Compression::default());
        let mut archive = tar::Builder::new(enc);
        let contents = b"router";
        let mut header = tar::Header::new_gnu();
        header.set_path(format!("{}{}", "dist/router", env::consts::EXE_SUFFIX))?;
        header.set_size(contents.len().try_into().unwrap());
        header.set_cksum();
        archive.append(&header, &contents[..]).unwrap();

        let finished_archive = archive.into_inner()?;
        let finished_archive_bytes = finished_archive.finish()?;

        http_server.mock(|when, then| {
            when.is_true(|request| {
                request.method() == Method::GET && request.uri().path().starts_with("/router")
            });
            then.status(200)
                .header("Content-Type", "application/octet-stream")
                .body(&finished_archive_bytes);
        });
        let binary = install_router
            .install(
                Utf8PathBuf::from_path_buf(override_install_path.to_path_buf()).ok(),
                license_accepter,
                false,
            )
            .await;
        let subject = assert_that!(binary).is_ok().subject;
        assert_that!(subject.version()).is_equal_to(&Version::from_str("1.57.1")?);

        let installed_binary_path = override_install_path.path().join(format!(
            "{}{}",
            ".rover/bin/router-v1.57.1",
            env::consts::EXE_SUFFIX
        ));
        assert_that!(subject.exe())
            .is_equal_to(&Utf8PathBuf::from_path_buf(installed_binary_path.clone()).unwrap());
        assert_that!(installed_binary_path.exists()).is_equal_to(true);
        let installed_binary_contents = std::fs::read(installed_binary_path)?;
        assert_that!(installed_binary_contents).is_equal_to(b"router".to_vec());
        Ok(())
    }
}
