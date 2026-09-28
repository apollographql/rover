use std::{
    env,
    io::{self, IsTerminal},
};

use bytes::Bytes;
use camino::Utf8PathBuf;
use download::{
    gz_decode::{GzDecodeError, GzDecodeLayer},
    FileDownloadService,
};
use http::Request;
use rover_http::{
    error_on_status::ErrorOnStatusLayer, Full, HttpRequest, HttpResponse, HttpServiceError,
};
use rover_std::Fs;
use tower::{Service, ServiceBuilder, ServiceExt};
use url::Url;

use crate::InstallerError;

pub mod download;

pub struct Installer {
    /// The name of the binary to install
    pub binary_name: String,
    /// Install without checking for existing installations, or to bypass TTY prompt
    pub force_install: bool,
    /// The location of the executable to be installed
    pub executable_location: Utf8PathBuf,
    /// Install the binary into a non-default location
    pub override_install_path: Option<Utf8PathBuf>,
}

impl Installer {
    /// Installs the executable and returns the location it was installed.
    pub fn install(&self) -> Result<Option<Utf8PathBuf>, InstallerError> {
        let bin_destination = self.get_binstall_path()?;

        if !self.force_install
            && bin_destination.exists()
            && !self.should_overwrite(&bin_destination, &self.binary_name)?
        {
            return Ok(None);
        }

        self.create_bin_dir()?;

        eprintln!("writing binary to {}", bin_destination);
        self.write_bin_to_fs()?;

        self.add_binary_to_path()?;

        Ok(Some(bin_destination))
    }

    /// The main tool should already be installed before calling this function
    ///
    /// Checks if a binary already exists, and if it does not,
    /// downloads a plugin tarball from a URL, extracts the binary,
    /// and puts it in the `bin` directory for the main tool
    ///
    /// `version` is the exact version the tarball holds, which names the installed binary. The
    /// caller resolves it — with [`Self::get_latest_plugin_version`] for a floating URL, or
    /// [`Self::get_plugin_version_from_url`] for an exact one — so it isn't resolved twice.
    pub async fn install_plugin(
        &self,
        plugin_name: &str,
        plugin_tarball_url: &str,
        file_download_service: FileDownloadService,
        version: &str,
    ) -> Result<Option<Utf8PathBuf>, InstallerError> {
        let bin_dir_path = self.get_bin_dir_path()?;
        if !bin_dir_path.exists() {
            Fs::create_dir_all(bin_dir_path)?;
        }

        let plugin_bin_destination = self.get_plugin_bin_path(plugin_name, version)?;
        if !self.force_install
            && plugin_bin_destination.exists()
            && !self.should_overwrite(&plugin_bin_destination, plugin_name)?
        {
            return Ok(None);
        }

        // Hold the extraction `TempDir` guard until the binary has been copied out
        let (_download_dir, plugin_bin_path) = self
            .extract_plugin_tarball(plugin_name, plugin_tarball_url, file_download_service)
            .await?;
        self.write_plugin_bin_to_fs(plugin_name, &plugin_bin_path, version)?;

        eprintln!(
            "the '{}' plugin was successfully installed to {}",
            plugin_name, plugin_bin_destination
        );

        Ok(Some(plugin_bin_destination))
    }

    /// Resolves the exact version a floating plugin tarball URL currently points at.
    ///
    /// The registry answers a `HEAD` on a floating URL with a redirect whose `X-Version` header
    /// names the release it resolves to. `version_service` must not follow redirects, or that
    /// header is lost; the caller owns retry and timeout policy by layering it onto the service.
    pub async fn get_latest_plugin_version<S>(
        &self,
        version_service: S,
        plugin_tarball_url: &str,
    ) -> Result<String, InstallerError>
    where
        S: Service<HttpRequest, Response = HttpResponse, Error = HttpServiceError>,
        S::Future: Send + 'static,
    {
        let request = Request::head(plugin_tarball_url)
            .body(Full::default())
            .map_err(HttpServiceError::from)
            .map_err(|source| InstallerError::VersionResolution {
                url: plugin_tarball_url.to_string(),
                source: Box::new(source),
            })?;
        let response = ServiceBuilder::new()
            .layer(ErrorOnStatusLayer::default())
            .service(version_service)
            .oneshot(request)
            .await
            .map_err(|source| InstallerError::VersionResolution {
                url: plugin_tarball_url.to_string(),
                source: Box::new(source),
            })?;

        if let Some(version) = response.headers().get("x-version") {
            Ok(version
                .to_str()
                .map_err(|e| InstallerError::IoError(io::Error::other(e)))?
                .to_string())
        } else {
            Err(InstallerError::IoError(io::Error::other(format!(
                "{plugin_tarball_url} did not respond with an X-Version header, which is required to determine the latest version"
            ))))
        }
    }

    /// Reads the version out of an exact plugin tarball URL, whose last path segment names it.
    pub fn get_plugin_version_from_url(
        &self,
        plugin_tarball_url: &str,
    ) -> Result<String, InstallerError> {
        let url = Url::parse(plugin_tarball_url).map_err(|e| {
            // this should be unreachable
            InstallerError::IoError(io::Error::new(io::ErrorKind::InvalidData, e))
        })?;
        if let Some(version) = url.path_segments().and_then(|mut s| s.next_back()) {
            if version.starts_with('v') {
                Ok(version.to_string())
            } else {
                Ok(format!("v{version}"))
            }
        } else {
            // this should be unreachable
            Err(InstallerError::IoError(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "The tarball url for the plugin ({plugin_tarball_url}) cannot be a base URL"
                ),
            )))
        }
    }

    /// Gets the location the executable will be installed to, creating it if needed
    pub fn get_bin_dir_path(&self) -> Result<Utf8PathBuf, InstallerError> {
        let (bin_dir, is_node_modules) = self.locate_bin_dir()?;
        if !is_node_modules {
            std::fs::create_dir_all(&bin_dir)?;
        }
        Ok(bin_dir)
    }

    /// Gets the location the executable will be installed to, without touching the filesystem,
    /// so that it can be named in an error even when it can't be created
    pub fn bin_dir_location(&self) -> Result<Utf8PathBuf, InstallerError> {
        Ok(self.locate_bin_dir()?.0)
    }

    /// The bin directory, and whether it's npm's `node_modules/.bin`, which Rover never creates.
    fn locate_bin_dir(&self) -> Result<(Utf8PathBuf, bool), InstallerError> {
        // TODO: loop this up better with rover's environment variable management
        if let Ok(node_modules_bin) = std::env::var("APOLLO_NODE_MODULES_BIN_DIR") {
            Ok((node_modules_bin.into(), true))
        } else {
            Ok((self.get_base_dir_path()?.join("bin"), false))
        }
    }

    pub(crate) fn get_base_dir_path(&self) -> Result<Utf8PathBuf, InstallerError> {
        let base_dir = if let Some(base_dir) = &self.override_install_path {
            Ok(base_dir.to_owned())
        } else {
            crate::get_home_dir_path()
        }?;
        Ok(base_dir.join(format!(".{}", self.binary_name)))
    }

    fn create_bin_dir(&self) -> Result<(), InstallerError> {
        tracing::debug!("Creating directory for binary");
        Fs::create_dir_all(self.get_bin_dir_path()?)?;
        Ok(())
    }

    fn get_binstall_path(&self) -> Result<Utf8PathBuf, InstallerError> {
        Ok(self
            .get_bin_dir_path()?
            .join(&self.binary_name)
            .with_extension(env::consts::EXE_EXTENSION))
    }

    fn get_plugin_bin_path(
        &self,
        plugin_name: &str,
        plugin_version: &str,
    ) -> Result<Utf8PathBuf, InstallerError> {
        let bin_dir_path = self.get_bin_dir_path()?;
        // we add the extra `.` at the end here so that `with_extension` does not replace
        // the patch version of the plugin with nothing on unix and .exe on windows.
        let plugin_name = format!("{plugin_name}-{plugin_version}.");
        let plugin_path = bin_dir_path
            .join(plugin_name)
            .with_extension(env::consts::EXE_EXTENSION);
        Ok(plugin_path)
    }

    fn write_bin_to_fs(&self) -> Result<(), InstallerError> {
        let binstall_path = self.get_binstall_path()?;
        Fs::copy(&self.executable_location, binstall_path)?;
        Ok(())
    }

    fn write_plugin_bin_to_fs(
        &self,
        plugin_name: &str,
        plugin_bin_path: &Utf8PathBuf,
        plugin_version: &str,
    ) -> Result<(), InstallerError> {
        let plugin_destination = self.get_plugin_bin_path(plugin_name, plugin_version)?;
        Fs::copy(plugin_bin_path, plugin_destination)?;
        Ok(())
    }

    fn should_overwrite(
        &self,
        destination: &Utf8PathBuf,
        binary_name: &str,
    ) -> Result<bool, InstallerError> {
        if &self.executable_location == destination {
            return Err(InstallerError::AlreadyInstalled);
        }

        // If we're not attached to a TTY then we can't get user input, so there's
        // nothing to do except inform the user about the `-f` flag.
        // TODO: abstract so this is testable
        if !std::io::stdin().is_terminal() {
            return Err(InstallerError::NoTty);
        }

        // It looks like we're at an interactive prompt, so ask the user if they'd
        // like to overwrite the previous installation.
        // TODO: abstract so this doesn't perform user IO deep in a subcommand
        eprintln!("existing {binary_name} installation found at `{destination}`");
        eprintln!("Would you like to overwrite this file? [y/N]: ");
        Ok(self.prompt_confirm()?)
    }

    fn prompt_confirm(&self) -> Result<bool, io::Error> {
        let mut line = String::new();
        io::stdin().read_line(&mut line)?;

        if line.to_lowercase().starts_with('y') {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Extracts a plugin tarball into a temp directory within the binary install directory.
    ///
    /// Returns both a handle to the temp directory and the extracted file path. The temp
    /// directory is cleaned up when the handle is dropped.
    async fn extract_plugin_tarball(
        &self,
        plugin_name: &str,
        plugin_tarball_url: &str,
        file_download_service: FileDownloadService,
    ) -> Result<(tempfile::TempDir, Utf8PathBuf), InstallerError> {
        // Extract into a temp dir within Rover's install directory rather than the system temp dir.
        // This lets a read-only root filesystem (or read-only `/tmp`) with a writable
        // `APOLLO_HOME` succeed instead of failing on the system temp dir (#1422).
        let bin_dir = self.get_bin_dir_path()?;
        let staging_dir = bin_dir.parent().unwrap_or(bin_dir.as_path());
        Fs::create_dir_all(staging_dir)?;
        let download_dir = tempfile::Builder::new()
            .prefix(plugin_name)
            .tempdir_in(staging_dir)?;
        let download_dir_path = Utf8PathBuf::try_from(download_dir.path().to_path_buf())?;
        let http_request = Request::builder()
            .method(http::Method::GET)
            .uri(plugin_tarball_url)
            .body(Full::new(Bytes::default()))
            .map_err(|err| anyhow::anyhow!(err))?;
        // Kept apart from the extraction errors below, so a caller can tell a
        // download that failed from an archive that did. A gzip stream that
        // ends early is a truncated transfer, so it counts as a download failure.
        let body_bytes = ServiceBuilder::new()
            .layer(GzDecodeLayer::default())
            .service(file_download_service.into_inner())
            .oneshot(http_request)
            .await
            .map_err(|err| match err {
                GzDecodeError::Decode(err) if err.kind() != io::ErrorKind::UnexpectedEof => {
                    InstallerError::UnpackArchive(err)
                }
                err => InstallerError::FileDownloadError(Box::new(err)),
            })?;
        let mut archive = tar::Archive::new(&body_bytes[..]);
        archive
            .unpack(&download_dir_path)
            .map_err(InstallerError::UnpackArchive)?;
        let path = download_dir_path.join("dist").join(format!(
            "{}{}",
            plugin_name,
            std::env::consts::EXE_SUFFIX
        ));
        Fs::assert_path_exists(&path)?;
        Ok((download_dir, path))
    }

    #[cfg(windows)]
    fn add_binary_to_path(&self) -> Result<(), InstallerError> {
        tracing::debug!("Adding binary to PATH");
        crate::windows::add_binary_to_path(self)
    }

    #[cfg(not(windows))]
    fn add_binary_to_path(&self) -> Result<(), InstallerError> {
        tracing::debug!("Adding binary to PATH");
        crate::unix::add_binary_to_path(self)
    }
}

#[cfg(test)]
mod test {
    use std::{
        env,
        io::{Read, Write},
        time::Duration,
    };

    use bytes::Bytes;
    use camino::Utf8PathBuf;
    use futures::future;
    use http::{HeaderValue, Response, Uri};
    use httpmock::prelude::*;
    use reqwest::header::{ACCEPT, USER_AGENT};
    use rover_http::{test::MockHttpService, Full, HttpService, HttpServiceError, ReqwestService};
    use rover_tower::{expect_poll_ready, test::MockCloneService};
    use rstest::{fixture, rstest};
    use sealed_test::prelude::*;
    use speculoos::prelude::*;
    use tower::ServiceExt;

    use super::Installer;
    use crate::{download::FileDownloadService, InstallerError};

    #[fixture]
    fn home_dir() -> Utf8PathBuf {
        let home_dir = std::env::home_dir().expect("No home_dir");
        Utf8PathBuf::from_path_buf(home_dir).expect("Unable to convert home_dir to Utf8PathBuf")
    }

    #[fixture]
    fn override_path() -> Utf8PathBuf {
        let override_path = tempfile::tempdir().expect("Unable to create temporary directory");
        let override_path = Utf8PathBuf::from_path_buf(override_path.path().to_path_buf())
            .expect("Unable to convert to Utf8PathBuf");
        override_path
    }

    #[fixture]
    #[once]
    fn binary_name() -> String {
        "test".to_string()
    }

    #[fixture]
    fn executable_location() -> Utf8PathBuf {
        let install_path = tempfile::tempdir().expect("Unable to create temporary directory");
        let install_path = Utf8PathBuf::from_path_buf(install_path.path().to_path_buf())
            .expect("Unable to convert to Utf8PathBuf");
        install_path
    }

    #[fixture]
    fn installer(executable_location: Utf8PathBuf, binary_name: &str) -> Installer {
        Installer {
            binary_name: binary_name.to_string(),
            force_install: true,
            executable_location,
            override_install_path: None,
        }
    }

    #[rstest]
    fn test_get_binstall_path(binary_name: &str, installer: Installer, home_dir: Utf8PathBuf) {
        let binstall_path = installer.get_binstall_path();
        let expected_install_subpath = format!(".{}", binary_name);
        let expected_extension = if cfg!(windows) { ".exe" } else { "" };
        let expected_binstall_path = home_dir
            .join(expected_install_subpath)
            .join("bin")
            .join(format!("{}{}", binary_name, expected_extension));
        assert_that!(binstall_path)
            .is_ok()
            .is_equal_to(expected_binstall_path);
    }

    #[rstest]
    fn test_get_bin_dir_path(binary_name: &str, installer: Installer, override_path: Utf8PathBuf) {
        let installer = Installer {
            override_install_path: Some(override_path.clone()),
            ..installer
        };
        let bin_dir_path = installer.get_bin_dir_path();
        let install_subpath = format!(".{}", binary_name);
        let mut bin_dir_path = assert_that!(bin_dir_path).is_ok();
        bin_dir_path.is_equal_to(override_path.join(install_subpath).join("bin"));
        assert_that!((*bin_dir_path.subject).exists()).is_true();
    }

    #[rstest]
    #[sealed_test]
    fn test_get_bin_dir_path_with_node_modules_override(
        installer: Installer,
        override_path: Utf8PathBuf,
    ) {
        std::env::set_var("APOLLO_NODE_MODULES_BIN_DIR", &override_path);
        let bin_dir_path = installer.get_bin_dir_path();
        assert_that!(bin_dir_path)
            .is_ok()
            .is_equal_to(override_path);
    }

    #[rstest]
    fn test_get_base_dir_path(binary_name: &str, installer: Installer, home_dir: Utf8PathBuf) {
        let expected_subpath = format!(".{}", binary_name);
        let base_dir_path = installer.get_base_dir_path();
        assert_that!(base_dir_path)
            .is_ok()
            .is_equal_to(home_dir.join(expected_subpath));
    }

    #[rstest]
    fn test_get_base_dir_path_with_override(
        binary_name: &str,
        installer: Installer,
        override_path: Utf8PathBuf,
    ) {
        let installer = Installer {
            override_install_path: Some(override_path.clone()),
            ..installer
        };
        let expected_subpath = format!(".{}", binary_name);
        let base_dir_path = installer.get_base_dir_path();
        assert_that!(base_dir_path)
            .is_ok()
            .is_equal_to(override_path.join(expected_subpath));
    }

    #[rstest]
    fn test_should_overwrite_at_executable_location(binary_name: &str, installer: Installer) {
        let executable_location = &installer.executable_location;
        let should_overwrite = installer.should_overwrite(executable_location, binary_name);
        assert_that!(should_overwrite)
            .is_err()
            .matches(|err| matches!(err, InstallerError::AlreadyInstalled));
    }

    #[rstest]
    fn test_create_bin_dir(binary_name: &str, installer: Installer, override_path: Utf8PathBuf) {
        let installer = Installer {
            override_install_path: Some(override_path.clone()),
            ..installer
        };
        installer
            .create_bin_dir()
            .expect("Failed to create bin dir");
        let expected_bin_dir = override_path.join(format!(".{}", binary_name)).join("bin");
        assert_that!(expected_bin_dir.exists()).is_true();
    }

    #[rstest]
    fn test_write_bin_to_fs(binary_name: &str, installer: Installer, override_path: Utf8PathBuf) {
        let installer = Installer {
            override_install_path: Some(override_path.clone()),
            ..installer
        };
        let executable_location = &installer.executable_location;
        let mut executable = std::fs::File::create(executable_location.as_std_path())
            .expect("Unable to create executable file");
        executable
            .write_all("test contents".as_bytes())
            .expect("Unable to write content to executable location");
        executable.flush().unwrap();
        installer
            .write_bin_to_fs()
            .expect("Failed to copy executable to target location");

        let expected_bin_path = override_path
            .join(format!(".{}", binary_name))
            .join("bin")
            .join(binary_name);
        let expected_bin_path = if cfg!(windows) {
            expected_bin_path.with_added_extension(env::consts::EXE_EXTENSION)
        } else {
            expected_bin_path
        };
        let bin_contents = std::fs::read_to_string(expected_bin_path)
            .expect("Unable to read from target location");
        assert_that!(bin_contents).is_equal_to("test contents".to_string());
    }

    /// The registry answers with a redirect, so resolution must not follow it.
    #[fixture]
    fn version_service() -> HttpService {
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        ReqwestService::builder()
            .client(client)
            .build()
            .unwrap()
            .boxed_clone()
    }

    #[rstest]
    #[case::ok(200)]
    #[case::registry_redirect(302)]
    #[tokio::test]
    async fn test_get_latest_plugin_version_with_valid_version(
        binary_name: &str,
        installer: Installer,
        version_service: HttpService,
        #[case] status: u16,
    ) {
        let server = MockServer::start();
        let address = server.address();
        let mock = server.mock(|when, then| {
            when.method(Method::HEAD).path(format!("/{}", binary_name));
            then.status(status)
                .header("x-version", "1.0.0")
                .header("location", "/elsewhere");
        });
        let tarball_url = format!("http://{}/{}", address, binary_name);
        let result = installer
            .get_latest_plugin_version(version_service, &tarball_url)
            .await;
        mock.assert_calls(1);
        assert_that!(result)
            .is_ok()
            .is_equal_to("1.0.0".to_string());
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_latest_plugin_version_with_error_status(
        binary_name: &str,
        installer: Installer,
        version_service: HttpService,
    ) {
        let server = MockServer::start();
        let address = server.address();
        let mock = server.mock(|when, then| {
            when.method(Method::HEAD).path(format!("/{}", binary_name));
            then.status(404);
        });
        let tarball_url = format!("http://{}/{}", address, binary_name);
        let result = installer
            .get_latest_plugin_version(version_service, &tarball_url)
            .await;
        mock.assert_calls(1);
        assert_that!(result).is_err().matches(|err| {
            matches!(
                err,
                InstallerError::VersionResolution {
                    url,
                    source,
                } if *url == tarball_url && matches!(
                    **source,
                    HttpServiceError::BadStatusCode { status_code, .. } if status_code.as_u16() == 404
                )
            )
        });
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_latest_plugin_version_with_invalid_version(
        binary_name: &str,
        installer: Installer,
        version_service: HttpService,
    ) {
        let server = MockServer::start();
        let address = server.address();
        let mock = server.mock(|when, then| {
            when.method(Method::HEAD).path(format!("/{}", binary_name));
            then.status(200);
        });
        let tarball_url = format!("http://{}/{}", address, binary_name);
        let result = installer
            .get_latest_plugin_version(version_service, &tarball_url)
            .await;
        mock.assert_calls(1);
        assert_that!(result)
            .is_err()
            .matches(|err| {
                err.to_string() == format!("{tarball_url} did not respond with an X-Version header, which is required to determine the latest version")
            });
    }

    #[rstest]
    #[case::with_v_prefix("v1.0.0", "v1.0.0")]
    #[case::without_v_prefix("1.0.0", "v1.0.0")]
    fn test_get_plugin_version_from_url_with_valid_version(
        binary_name: &str,
        installer: Installer,
        #[case] tarball_version_str: &str,
        #[case] expected_version: &str,
    ) {
        let tarball_url = format!("http://example.com/{}/{}", binary_name, tarball_version_str);
        let result = installer.get_plugin_version_from_url(&tarball_url);
        assert_that!(result)
            .is_ok()
            .is_equal_to(expected_version.to_string());
    }

    #[rstest]
    fn test_get_plugin_bin_path(
        binary_name: &str,
        installer: Installer,
        override_path: Utf8PathBuf,
    ) {
        let installer = Installer {
            override_install_path: Some(override_path.clone()),
            ..installer
        };
        let plugin_name = "my-plugin";
        let plugin_version = "v1.0.0";
        let install_subpath = format!(".{}", binary_name);
        let bin_path = Utf8PathBuf::from(format!("{plugin_name}-{plugin_version}"));
        let bin_path = if cfg!(windows) {
            bin_path.with_added_extension(env::consts::EXE_EXTENSION)
        } else {
            bin_path
        };
        let expected_bin_path = override_path
            .join(install_subpath)
            .join("bin")
            .join(bin_path);
        assert_that!(installer.get_plugin_bin_path(plugin_name, plugin_version))
            .is_ok()
            .is_equal_to(expected_bin_path);
    }

    #[rstest]
    #[tokio::test]
    async fn test_extract_plugin_tarball(
        binary_name: &str,
        installer: Installer,
        override_path: Utf8PathBuf,
    ) {
        let installer = Installer {
            override_install_path: Some(override_path),
            ..installer
        };

        let mut plugin_tempfile = tempfile::NamedTempFile::new().unwrap();
        plugin_tempfile.write_all("contents".as_bytes()).unwrap();
        plugin_tempfile.flush().unwrap();
        let mut builder = tar::Builder::new(Vec::new());
        let binary_path = if cfg!(windows) { "test.exe" } else { "test" };
        builder
            .append_path_with_name(plugin_tempfile.path(), format!("dist/{}", binary_path))
            .unwrap();
        let tar_bytes = builder.into_inner().unwrap();
        let mut gzip_encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip_encoder.write_all(&tar_bytes).unwrap();
        let gzipped_tar = gzip_encoder.finish().unwrap();

        let tarball_url = format!("http://example.com/{}", binary_name);
        let mut mock_http_service = MockHttpService::new();
        expect_poll_ready!(mock_http_service);
        mock_http_service
            .expect_call()
            .times(1)
            .withf({
                let tarball_url = tarball_url.clone();
                move |req| {
                    let headers = req.headers();
                    let headers_match = headers.get(USER_AGENT)
                        == Some(&HeaderValue::from_static("rover-client"))
                        && headers.get(ACCEPT)
                            == Some(&HeaderValue::from_static("application/octet-stream"));
                    headers_match
                        && req.method() == &Method::GET
                        && req.uri() == &Uri::try_from(&tarball_url).unwrap()
                }
            })
            .returning(move |_| {
                future::ready(
                    Response::builder()
                        .status(200)
                        .body(Full::new(Bytes::from(gzipped_tar.clone())))
                        .map_err(HttpServiceError::from),
                )
            });
        let service = FileDownloadService::builder()
            .http_service(MockCloneService::new(mock_http_service))
            .max_elapsed_duration(Duration::from_secs(5))
            .timeout_duration(Duration::from_secs(1))
            .build();
        let result = installer
            .extract_plugin_tarball(binary_name, &tarball_url, service)
            .await;
        let (_tempdir, plugin_path) = result.expect("extract_plugin_tarball should succeed");
        assert_that!(plugin_path).starts_with(
            installer
                .get_bin_dir_path()
                .unwrap()
                .parent()
                .and_then(|parent| parent.parent())
                .expect("test installer dir should be at a non-root directory"),
        );
        let mut contents = String::new();
        let mut f = std::fs::File::open(&plugin_path).unwrap();
        f.read_to_string(&mut contents).unwrap();
        assert_that!(contents).is_equal_to("contents".to_string());
    }

    #[rstest]
    #[tokio::test]
    async fn test_extract_plugin_tarball_errors_on_non_success_status(
        binary_name: &str,
        installer: Installer,
        override_path: Utf8PathBuf,
    ) {
        let installer = Installer {
            override_install_path: Some(override_path),
            ..installer
        };

        let tarball_url = format!("http://example.com/{}", binary_name);
        let mut mock_http_service = MockHttpService::new();
        expect_poll_ready!(mock_http_service);
        mock_http_service
            .expect_call()
            // 401 isn't retryable, so we expect exactly one call to the inner service.
            .times(1)
            .returning(move |_| {
                future::ready(
                    Response::builder()
                        .status(401)
                        .body(Full::new(Bytes::from_static(
                            b"<html>401 Unauthorized</html>",
                        )))
                        .map_err(HttpServiceError::from),
                )
            });
        let service = FileDownloadService::builder()
            .http_service(MockCloneService::new(mock_http_service))
            .max_elapsed_duration(Duration::from_secs(5))
            .timeout_duration(Duration::from_secs(1))
            .build();

        let err = installer
            .extract_plugin_tarball(binary_name, &tarball_url, service)
            .await
            .expect_err("a 401 response must produce an error");
        let rendered = rover_std::format_error_chain(&err);

        // A download failure, reported as one — not as the gzip decode of the
        // error body that a bad status would otherwise run into.
        assert_that!((rendered, err.download_status())).is_equal_to((
            "Couldn't download the plugin artifact: Bad Status code: 401 Unauthorized".to_string(),
            Some(http::StatusCode::UNAUTHORIZED),
        ));
    }

    /// An artifact that downloads fine but isn't a gzipped tarball is an
    /// unpacking failure, not a download one: retrying the download can't fix
    /// what the registry served.
    #[rstest]
    #[case::not_gzip(
        Bytes::from_static(b"<html>not a tarball</html>"),
        "invalid gzip header"
    )]
    #[case::gzip_but_not_tar(
        gzipped(b"not a tar archive"),
        "failed to iterate over archive: failed to read entire block"
    )]
    #[tokio::test]
    async fn test_extract_plugin_tarball_reports_an_unreadable_archive_as_unpacking(
        binary_name: &str,
        installer: Installer,
        override_path: Utf8PathBuf,
        #[case] body: Bytes,
        #[case] cause: &str,
    ) {
        let installer = Installer {
            override_install_path: Some(override_path),
            ..installer
        };

        let tarball_url = format!("http://example.com/{}", binary_name);
        let mut mock_http_service = MockHttpService::new();
        expect_poll_ready!(mock_http_service);
        mock_http_service
            .expect_call()
            .times(1)
            .returning(move |_| {
                future::ready(
                    Response::builder()
                        .status(200)
                        .body(Full::new(body.clone()))
                        .map_err(HttpServiceError::from),
                )
            });
        let service = FileDownloadService::builder()
            .http_service(MockCloneService::new(mock_http_service))
            .max_elapsed_duration(Duration::from_secs(5))
            .timeout_duration(Duration::from_secs(1))
            .build();

        let err = installer
            .extract_plugin_tarball(binary_name, &tarball_url, service)
            .await
            .expect_err("an unreadable archive must produce an error");

        assert_that!(rover_std::format_error_chain(&err)).is_equal_to(format!(
            "Couldn't unpack the downloaded plugin archive: {cause}"
        ));
    }

    /// A gzip stream that stops early is a transfer that was cut off, which a
    /// retried download can fix, so it's reported as a download failure.
    #[rstest]
    #[tokio::test]
    async fn test_extract_plugin_tarball_reports_a_truncated_download_as_downloading(
        binary_name: &str,
        installer: Installer,
        override_path: Utf8PathBuf,
    ) {
        let installer = Installer {
            override_install_path: Some(override_path),
            ..installer
        };
        let whole = gzipped(b"a tarball that the transfer cuts off partway through");
        let truncated = whole.slice(..whole.len() / 2);

        let mut mock_http_service = MockHttpService::new();
        expect_poll_ready!(mock_http_service);
        mock_http_service
            .expect_call()
            .times(1)
            .returning(move |_| {
                future::ready(
                    Response::builder()
                        .status(200)
                        .body(Full::new(truncated.clone()))
                        .map_err(HttpServiceError::from),
                )
            });
        let service = FileDownloadService::builder()
            .http_service(MockCloneService::new(mock_http_service))
            .max_elapsed_duration(Duration::from_secs(5))
            .timeout_duration(Duration::from_secs(1))
            .build();

        let err = installer
            .extract_plugin_tarball(binary_name, "http://example.com/test", service)
            .await
            .expect_err("a truncated archive must produce an error");

        assert_that!((rover_std::format_error_chain(&err), err.download_status())).is_equal_to((
            "Couldn't download the plugin artifact: Failed to decode file: incomplete deflate stream: incomplete deflate stream"
                .to_string(),
            None,
        ));
    }

    /// Naming the install root must not create it: a caller names it in the
    /// error for a root that couldn't be created.
    #[rstest]
    fn test_bin_dir_location_does_not_create_it(
        binary_name: &str,
        installer: Installer,
        override_path: Utf8PathBuf,
    ) {
        let installer = Installer {
            override_install_path: Some(override_path.clone()),
            ..installer
        };

        let location = installer.bin_dir_location().unwrap();

        assert_that!((location.clone(), location.exists())).is_equal_to((
            override_path.join(format!(".{binary_name}")).join("bin"),
            false,
        ));
    }

    fn gzipped(contents: &[u8]) -> Bytes {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(contents).unwrap();
        Bytes::from(encoder.finish().unwrap())
    }

    #[rstest]
    fn test_write_plugin_bin_to_fs(
        binary_name: &str,
        installer: Installer,
        override_path: Utf8PathBuf,
    ) -> anyhow::Result<()> {
        let installer = Installer {
            override_install_path: Some(override_path.clone()),
            ..installer
        };

        let plugin_name = "my-plugin";
        let plugin_version = "v1.0.0";
        let install_subpath = format!(".{}", binary_name);
        let bin_path = Utf8PathBuf::from(format!("{plugin_name}-{plugin_version}"));
        let bin_path = if cfg!(windows) {
            bin_path.with_added_extension(env::consts::EXE_EXTENSION)
        } else {
            bin_path
        };
        let expected_bin_path = override_path
            .join(install_subpath)
            .join("bin")
            .join(bin_path);

        let plugin_tempdir = tempfile::tempdir().unwrap();
        let dist_dir = plugin_tempdir.path().join("dist");
        std::fs::create_dir_all(&dist_dir).unwrap();
        let plugin_bin_file = dist_dir.join(plugin_name);
        std::fs::write(&plugin_bin_file, "contents").unwrap();
        let plugin_bin_path = Utf8PathBuf::from_path_buf(plugin_bin_file)
            .expect("Unable to convert PathBuf to Utf8PathBuf");

        let result =
            installer.write_plugin_bin_to_fs(plugin_name, &plugin_bin_path, plugin_version);
        assert_that!(result).is_ok();
        let mut written_plugin = std::fs::File::open(expected_bin_path).unwrap();
        let mut plugin_contents = String::new();
        written_plugin.read_to_string(&mut plugin_contents).unwrap();
        assert_that!(plugin_contents).is_equal_to("contents".to_string());
        Ok(())
    }
}
