use std::env;
use std::io::{self, IsTerminal, Write};

use camino::Utf8PathBuf;
use url::Url;

use rover_std::Fs;

use crate::InstallerError;

pub struct Installer {
    pub binary_name: String,
    pub force_install: bool,
    pub executable_location: Utf8PathBuf,
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
                    .map_err(|e| InstallerError::IoError(io::Error::new(io::ErrorKind::Other, e)))?
                    .to_string())
            } else {
                Err(InstallerError::IoError(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "{} did not respond with an X-Version header, which is required to determine the latest version",
                        plugin_tarball_url
                    ),
                )))
            }
        } else {
            let url = Url::parse(plugin_tarball_url).map_err(|e| {
                // this should be unreachable
                InstallerError::IoError(io::Error::new(io::ErrorKind::InvalidData, e))
            })?;
            if let Some(version) = url.path_segments().and_then(|s| s.last()) {
                if version.starts_with('v') {
                    Ok(version.to_string())
                } else {
                    Ok(format!("v{version}"))
                }
            } else {
                // this should be unreachable
                Err(InstallerError::IoError(io::Error::new(io::ErrorKind::InvalidData, format!("The tarball url for the plugin ({plugin_tarball_url}) cannot be a base URL"))))
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
        let plugin_name = format!("{}-{}.", plugin_name, plugin_version);
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
                    eprintln!("WARN: {:?}", e);
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
        if !std::io::stdin().is_terminal() {
            return Err(InstallerError::NoTty);
        }

        // It looks like we're at an interactive prompt, so ask the user if they'd
        // like to overwrite the previous installation.
        eprintln!(
            "existing {} installation found at `{}`",
            binary_name, destination
        );
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
        let download_dir_path = Utf8PathBuf::try_from(download_dir.into_path())?;
        let tarball_path = download_dir_path.join(format!("{}.tar.gz", plugin_name));
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
