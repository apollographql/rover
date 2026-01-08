use std::{
    env,
    io::{self, IsTerminal, Write},
};

use camino::Utf8PathBuf;
use rover_std::Fs;
use url::Url;

use crate::InstallerError;

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

        eprintln!("writing binary to {}", &bin_destination);
        self.write_bin_to_fs()?;

        self.add_binary_to_path()?;

        Ok(Some(bin_destination))
    }

    /// The main tool should already be installed before calling this function
    ///
    /// Checks if a binary already exists, and if it does not,
    /// downloads a plugin tarball from a URL, extracts the binary,
    /// and puts it in the `bin` directory for the main tool
    pub async fn install_plugin(
        &self,
        plugin_name: &str,
        plugin_tarball_url: &str,
        client: &reqwest::Client,
        is_latest: bool,
    ) -> Result<Option<Utf8PathBuf>, InstallerError> {
        let version = self
            .get_plugin_version(plugin_tarball_url, is_latest)
            .await?;

        let bin_dir_path = self.get_bin_dir_path()?;
        if !bin_dir_path.exists() {
            Fs::create_dir_all(bin_dir_path)?;
        }

        let plugin_bin_destination = self.get_plugin_bin_path(plugin_name, &version)?;
        if !self.force_install
            && plugin_bin_destination.exists()
            && !self.should_overwrite(&plugin_bin_destination, plugin_name)?
        {
            return Ok(None);
        }

        let plugin_bin_path = self
            .extract_plugin_tarball(plugin_name, plugin_tarball_url, client)
            .await?;
        self.write_plugin_bin_to_fs(plugin_name, &plugin_bin_path, &version)?;

        eprintln!(
            "the '{}' plugin was successfully installed to {}",
            &plugin_name, &plugin_bin_destination
        );

        Ok(Some(plugin_bin_destination))
    }

    pub async fn get_plugin_version(
        &self,
        plugin_tarball_url: &str,
        is_latest: bool,
    ) -> Result<String, InstallerError> {
        if is_latest {
            let no_redirect_client = reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()?;
            let response = no_redirect_client
                .head(plugin_tarball_url)
                .send()
                .await?
                .error_for_status()?;

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
        } else {
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
    }

    /// Gets the location the executable will be installed to
    pub fn get_bin_dir_path(&self) -> Result<Utf8PathBuf, InstallerError> {
        // TODO: loop this up better with rover's environment variable management
        let bin_dir = if let Ok(node_modules_bin) = std::env::var("APOLLO_NODE_MODULES_BIN_DIR") {
            node_modules_bin.into()
        } else {
            let bin_dir = self.get_base_dir_path()?.join("bin");
            std::fs::create_dir_all(&bin_dir)?;
            bin_dir
        };
        Ok(bin_dir)
    }

    pub(crate) fn get_base_dir_path(&self) -> Result<Utf8PathBuf, InstallerError> {
        let base_dir = if let Some(base_dir) = &self.override_install_path {
            Ok(base_dir.to_owned())
        } else {
            crate::get_home_dir_path()
        }?;
        Ok(base_dir.join(format!(".{}", &self.binary_name)))
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
        // clean up temp dir
        if let Some(dist) = plugin_bin_path.parent() {
            if let Some(tempdir) = dist.parent() {
                // attempt to clean up the temp dir
                // but do not error if it doesn't exist or something goes wrong
                if let Err(e) = Fs::remove_dir_all(tempdir) {
                    eprintln!("WARN: {e:?}");
                }
            }
        }
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

    async fn extract_plugin_tarball(
        &self,
        plugin_name: &str,
        plugin_tarball_url: &str,
        client: &reqwest::Client,
    ) -> Result<Utf8PathBuf, InstallerError> {
        let download_dir = tempfile::Builder::new().prefix(plugin_name).tempdir()?;
        let download_dir_path = Utf8PathBuf::try_from(download_dir.keep())?;
        let tarball_path = download_dir_path.join(format!("{plugin_name}.tar.gz"));
        let mut f = std::fs::File::create(&tarball_path)?;
        let response_bytes = client
            .get(plugin_tarball_url)
            .header(reqwest::header::USER_AGENT, "rover-client")
            .header(reqwest::header::ACCEPT, "application/octet-stream")
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        f.write_all(&response_bytes[..])?;
        f.sync_all()?;
        let f = std::fs::File::open(&tarball_path)?;
        let tar = flate2::read::GzDecoder::new(f);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(&download_dir_path)?;
        let path = download_dir_path.join("dist").join(format!(
            "{}{}",
            plugin_name,
            std::env::consts::EXE_SUFFIX
        ));
        Fs::assert_path_exists(&path)?;
        Ok(path)
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

    use camino::Utf8PathBuf;
    use httpmock::prelude::*;
    use reqwest::header::{ACCEPT, USER_AGENT};
    use rstest::{fixture, rstest};
    use sealed_test::prelude::*;
    use speculoos::prelude::*;

    use super::Installer;
    use crate::InstallerError;

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
        let bin_contents = std::fs::read_to_string(expected_bin_path)
            .expect("Unable to read from target location");
        assert_that!(bin_contents).is_equal_to("test contents".to_string());
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_plugin_version_latest_with_valid_version(
        binary_name: &str,
        installer: Installer,
    ) {
        let server = MockServer::start();
        let address = server.address();
        let mock = server.mock(|when, then| {
            when.method(Method::HEAD).path(format!("/{}", binary_name));
            then.status(200).header("x-version", "1.0.0");
        });
        let tarball_url = format!("http://{}/{}", address, binary_name);
        let result = installer.get_plugin_version(&tarball_url, true).await;
        mock.assert_calls(1);
        assert_that!(result)
            .is_ok()
            .is_equal_to("1.0.0".to_string());
    }

    #[rstest]
    #[tokio::test]
    async fn test_get_plugin_version_latest_with_invalid_version(
        binary_name: &str,
        installer: Installer,
    ) {
        let server = MockServer::start();
        let address = server.address();
        let mock = server.mock(|when, then| {
            when.method(Method::HEAD).path(format!("/{}", binary_name));
            then.status(200);
        });
        let tarball_url = format!("http://{}/{}", address, binary_name);
        let result = installer.get_plugin_version(&tarball_url, true).await;
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
    #[tokio::test]
    async fn test_get_plugin_version_with_valid_version(
        binary_name: &str,
        installer: Installer,
        #[case] tarball_version_str: &str,
        #[case] expected_version: &str,
    ) {
        let tarball_url = format!("http://example.com/{}/{}", binary_name, tarball_version_str);
        let result = installer.get_plugin_version(&tarball_url, false).await;
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
            bin_path.with_extension(env::consts::EXE_EXTENSION)
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
        builder
            .append_path_with_name(plugin_tempfile.path(), "dist/test")
            .unwrap();
        let tar_bytes = builder.into_inner().unwrap();
        let mut gzip_encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip_encoder.write_all(&tar_bytes).unwrap();
        let gzipped_tar = gzip_encoder.finish().unwrap();

        let server = MockServer::start();
        let address = server.address();
        let _mock = server.mock(|when, then| {
            when.method(Method::GET)
                .path(format!("/{}", binary_name))
                .header(USER_AGENT.as_str(), "rover-client")
                .header(ACCEPT.as_str(), "application/octet-stream");
            then.status(200).body(&gzipped_tar[..]);
        });
        let tarball_url = format!("http://{}/{}", address, binary_name);
        let client = reqwest::Client::builder()
            .gzip(true)
            .brotli(true)
            .timeout(Duration::from_secs(1))
            .build()
            .unwrap();
        let result = installer
            .extract_plugin_tarball(binary_name, &tarball_url, &client)
            .await;
        let plugin_path = assert_that!(result).is_ok().subject.clone();
        let mut contents = String::new();
        let mut f = std::fs::File::open(&plugin_path).unwrap();
        f.read_to_string(&mut contents).unwrap();
        assert_that!(contents).is_equal_to("contents".to_string());
    }

    #[rstest]
    fn test_write_plugin_bin_to_fs(
        binary_name: &str,
        installer: Installer,
        override_path: Utf8PathBuf,
    ) {
        let installer = Installer {
            override_install_path: Some(override_path.clone()),
            ..installer
        };

        let mut plugin_bin = tempfile::NamedTempFile::new().unwrap();
        plugin_bin.write_all("contents".as_bytes()).unwrap();
        plugin_bin.flush().unwrap();

        let plugin_name = "my-plugin";
        let plugin_version = "v1.0.0";
        let install_subpath = format!(".{}", binary_name);
        let bin_path = Utf8PathBuf::from(format!("{plugin_name}-{plugin_version}"));
        let bin_path = if cfg!(windows) {
            bin_path.with_extension(env::consts::EXE_EXTENSION)
        } else {
            bin_path
        };
        let expected_bin_path = override_path
            .join(install_subpath)
            .join("bin")
            .join(bin_path);

        let plugin_bin_path = Utf8PathBuf::from_path_buf(plugin_bin.path().to_path_buf())
            .expect("Unable to convert PathBuf to Utf8PathBuf");
        let result =
            installer.write_plugin_bin_to_fs(plugin_name, &plugin_bin_path, plugin_version);
        assert_that!(result).is_ok();
        let mut written_plugin = std::fs::File::open(expected_bin_path).unwrap();
        let mut plugin_contents = String::new();
        written_plugin.read_to_string(&mut plugin_contents).unwrap();
        assert_that!(plugin_contents).is_equal_to("contents".to_string());
    }
}
