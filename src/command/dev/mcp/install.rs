use async_trait::async_trait;
use camino::{Utf8Path, Utf8PathBuf};
use semver::Version;

use super::binary::McpServerBinary;
use crate::command::install::McpServerVersion;
use crate::{
    command::{Install, install::Plugin},
    options::LicenseAccepter,
    utils::{client::StudioClientConfig, effect::install::InstallBinary},
};

#[derive(thiserror::Error, Debug)]
#[error("Failed to install the MCP Sever")]
pub enum InstallMcpServerError {
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

pub struct InstallMcpServer {
    studio_client_config: StudioClientConfig,
    mcp_server_version: McpServerVersion,
}

impl InstallMcpServer {
    #[allow(unused)]
    pub const fn new(
        mcp_server_version: McpServerVersion,
        studio_client_config: StudioClientConfig,
    ) -> InstallMcpServer {
        InstallMcpServer {
            mcp_server_version,
            studio_client_config,
        }
    }
}

#[async_trait]
impl InstallBinary for InstallMcpServer {
    type Binary = McpServerBinary;
    type Error = InstallMcpServerError;
    async fn install(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool,
    ) -> Result<Self::Binary, Self::Error> {
        let plugin = Plugin::McpServer(self.mcp_server_version.clone());
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
            .map_err(|err| InstallMcpServerError::MissingDependency {
                err: err.to_string(),
            })?;
        let version = version_from_path(&exe)?;
        let binary = McpServerBinary::new(exe, version);
        Ok(binary)
    }
}

fn version_from_path(path: &Utf8Path) -> Result<Version, InstallMcpServerError> {
    let file_name = path
        .file_name()
        .ok_or_else(|| InstallMcpServerError::MissingFilename {
            path: path.to_path_buf(),
        })?;
    let without_exe = file_name.strip_suffix(".exe").unwrap_or(file_name);
    let without_prefix = without_exe
        .strip_prefix("apollo-mcp-server-v")
        .unwrap_or(without_exe);
    let version = Version::parse(without_prefix).map_err(|err| InstallMcpServerError::Semver {
        input: without_prefix.to_string(),
        source: err,
    })?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use std::{env, str::FromStr, time::Duration};

    use crate::command::install::McpServerVersion;
    use anyhow::Result;
    use assert_fs::{NamedTempFile, TempDir};
    use camino::Utf8PathBuf;
    use flate2::{Compression, write::GzEncoder};
    use houston::Config;
    use http::Method;
    use httpmock::MockServer;
    use rstest::{fixture, rstest};
    use semver::Version;
    use speculoos::prelude::*;
    use tracing_test::traced_test;

    use super::InstallMcpServer;
    use crate::{
        options::LicenseAccepter,
        utils::{
            client::{ClientBuilder, ClientTimeout, StudioClientConfig},
            effect::install::InstallBinary,
        },
    };

    #[fixture]
    #[once]
    fn http_server() -> MockServer {
        MockServer::start()
    }

    //noinspection HttpUrlsUsage
    #[fixture]
    #[once]
    fn mock_server_endpoint(http_server: &MockServer) -> String {
        let address = http_server.address();
        let endpoint = format!("http://{address}");
        endpoint
    }

    #[fixture]
    #[once]
    fn home() -> TempDir {
        TempDir::new().unwrap()
    }

    #[fixture]
    fn mcp_server_version() -> McpServerVersion {
        McpServerVersion::Latest
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
            ClientTimeout::default(),
        )
    }

    #[traced_test]
    #[tokio::test]
    #[rstest]
    #[timeout(Duration::from_secs(15))]
    async fn test_install(
        mcp_server_version: McpServerVersion,
        studio_client_config: StudioClientConfig,
        http_server: &MockServer,
        mock_server_endpoint: &str,
    ) -> Result<()> {
        let license_accepter = LicenseAccepter {
            elv2_license_accepted: Some(true),
        };
        let override_install_path = NamedTempFile::new("override_path")?;
        let install_mcp_server = InstallMcpServer::new(mcp_server_version, studio_client_config);
        http_server.mock(|when, then| {
            when.is_true(|request| {
                request.method() == Method::HEAD
                    && request.uri().path().starts_with("/tar/apollo-mcp-server")
            });
            then.status(302).header("X-Version", "v0.1.0");
        });
        http_server.mock(|when, then| {
            when.is_true(|request| {
                request.method() == Method::GET
                    && request.uri().path().starts_with("/tar/apollo-mcp-server/")
            });
            then.status(302).header(
                "Location",
                format!("{mock_server_endpoint}/apollo-mcp-server/"),
            );
        });

        let enc = GzEncoder::new(Vec::new(), Compression::default());
        let mut archive = tar::Builder::new(enc);
        let contents = b"apollo-mcp-server";
        let mut header = tar::Header::new_gnu();
        header.set_path(format!(
            "{}{}",
            "dist/apollo-mcp-server",
            env::consts::EXE_SUFFIX
        ))?;
        header.set_size(contents.len().try_into().unwrap());
        header.set_cksum();
        archive.append(&header, &contents[..]).unwrap();

        let finished_archive = archive.into_inner()?;
        let finished_archive_bytes = finished_archive.finish()?;

        http_server.mock(|when, then| {
            when.is_true(|request| {
                request.method() == Method::GET
                    && request.uri().path().starts_with("/apollo-mcp-server")
            });
            then.status(200)
                .header("Content-Type", "application/octet-stream")
                .body(&finished_archive_bytes);
        });
        let binary = temp_env::async_with_vars(
            [("APOLLO_ROVER_DOWNLOAD_HOST", Some(mock_server_endpoint))],
            async {
                install_mcp_server
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
        assert_that!(subject.version()).is_equal_to(&Version::from_str("0.1.0")?);

        let installed_binary_path = override_install_path.path().join(format!(
            "{}{}",
            ".rover/bin/apollo-mcp-server-v0.1.0",
            env::consts::EXE_SUFFIX
        ));
        assert_that!(subject.exe())
            .is_equal_to(&Utf8PathBuf::from_path_buf(installed_binary_path.clone()).unwrap());
        assert_that!(installed_binary_path.exists()).is_equal_to(true);
        let installed_binary_contents = std::fs::read(installed_binary_path)?;
        assert_that!(installed_binary_contents).is_equal_to(b"apollo-mcp-server".to_vec());
        Ok(())
    }
}
