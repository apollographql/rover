use crate::InstallerError;

use std::env;
use std::fs;
use std::io::{self, Write};

use atty::{self, Stream};
use camino::Utf8PathBuf;

pub struct Installer {
    pub binary_name: String,
    pub force_install: bool,
    pub executable_location: Utf8PathBuf,
    pub override_install_path: Option<Utf8PathBuf>,
}

impl Installer {
    /// Installs the executable and returns the location it was installed.
    pub fn install(&self) -> Result<Option<Utf8PathBuf>, InstallerError> {
        let bin_destination = self.get_bin_path()?;

        if !self.force_install
            && bin_destination.exists()
            && !self.should_overwrite(&bin_destination, &self.binary_name)?
        {
            return Ok(None);
        }

        self.create_bin_dir()?;

        eprintln!("Writing binary to {}", &bin_destination);
        self.write_bin_to_fs()?;

        self.add_binary_to_path()?;

        Ok(Some(bin_destination))
    }

    /// This command requires that the binary already exists,
    /// Downloads a plugin tarball from a URL, extracts the binary,
    /// and puts it in the `bin` directory for the main tool
    pub fn install_plugin(
        &self,
        plugin_name: &str,
        plugin_tarball_url: &str,
        requires_elv2_license: bool,
        accept_elv2_license: bool,
        client: &reqwest::blocking::Client,
        version: Option<String>,
    ) -> Result<Option<Utf8PathBuf>, InstallerError> {
        let version = if let Some(version) = version {
            Ok(version)
        } else {
            self.get_plugin_version(plugin_tarball_url)
        }?;
        if requires_elv2_license && !accept_elv2_license {
            eprintln!("{} is licensed under the Elastic license, the full text can be found here: https://raw.githubusercontent.com/apollographql/rover/{}/plugins/{}/LICENSE", plugin_name, &version, plugin_name);
            eprintln!("By installing this plugin, you accept the terms and conditions outlined by this license.");
            self.prompt_accept_elv2_license()?;
        }
        if self.get_bin_dir_path()?.exists() {
            // The main binary already exists in a standard location
            let plugin_bin_destination = self.get_plugin_bin_path(plugin_name, &version)?;
            if !self.force_install
                && plugin_bin_destination.exists()
                && !self.should_overwrite(&plugin_bin_destination, plugin_name)?
            {
                return Ok(None);
            }
            let plugin_bin_path =
                self.extract_plugin_tarball(plugin_name, plugin_tarball_url, client)?;
            self.write_plugin_bin_to_fs(plugin_name, &plugin_bin_path, &version)?;
            Ok(Some(plugin_bin_destination))
        } else {
            Err(InstallerError::PluginRequiresTool {
                plugin: plugin_name.to_string(),
                tool: self.binary_name.to_string(),
            })
        }
    }

    pub fn get_plugin_version(&self, plugin_tarball_url: &str) -> Result<String, InstallerError> {
        let no_redirect_client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let response = no_redirect_client
            .head(plugin_tarball_url)
            .send()?
            .error_for_status()?;
        Ok(response
            .headers()
            .get("x-version")
            .ok_or_else(|| {
                InstallerError::IoError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "{} did not respond with an X-Version header",
                        plugin_tarball_url
                    ),
                ))
            })?
            .to_str()
            .map_err(|e| {
                InstallerError::IoError(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?
            .to_string())
    }

    /// Gets the location the executable will be installed to
    pub fn get_bin_dir_path(&self) -> Result<Utf8PathBuf, InstallerError> {
        // TODO: loop this up better with rover's environment variable management
        let bin_dir = if let Ok(node_modules_bin) = std::env::var("APOLLO_NODE_MODULES_BIN_DIR") {
            node_modules_bin.into()
        } else {
            self.get_base_dir_path()?.join("bin")
        };
        Ok(bin_dir)
    }

    pub(crate) fn get_base_dir_path(&self) -> Result<Utf8PathBuf, InstallerError> {
        let base_dir = if let Some(base_dir) = &self.override_install_path {
            Ok(base_dir.to_owned())
        } else {
            crate::get_home_dir_path()
        }?;
        Ok(base_dir.join(&format!(".{}", &self.binary_name)))
    }

    fn create_bin_dir(&self) -> Result<(), InstallerError> {
        tracing::debug!("Creating directory for binary");
        fs::create_dir_all(self.get_bin_dir_path()?)?;
        Ok(())
    }

    fn get_bin_path(&self) -> Result<Utf8PathBuf, InstallerError> {
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
        let bin_path = self.get_bin_path()?;
        tracing::debug!(
            "copying \"{}\" to \"{}\"",
            &self.executable_location,
            &bin_path
        );
        // attempt to remove the old binary
        // but do not error if it doesn't exist.
        let _ = fs::remove_file(&bin_path);
        fs::copy(&self.executable_location, &bin_path)?;
        Ok(())
    }

    fn write_plugin_bin_to_fs(
        &self,
        plugin_name: &str,
        plugin_bin_path: &Utf8PathBuf,
        plugin_version: &str,
    ) -> Result<(), InstallerError> {
        let plugin_destination = self.get_plugin_bin_path(plugin_name, plugin_version)?;
        // attempt to remove the old binary
        // but do not error if it doesn't exist.
        let _ = fs::remove_file(&plugin_destination);
        fs::copy(plugin_bin_path, &plugin_destination)?;
        // clean up temp dir
        if let Some(dist) = plugin_bin_path.parent() {
            if let Some(tempdir) = dist.parent() {
                // attempt to clean up the temp dir
                // but do not error if it doesn't exist or something goes wrong
                if let Err(e) = fs::remove_dir_all(tempdir) {
                    eprintln!("WARN: could not remove {}: {}", tempdir, e);
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
        // If we're not attached to a TTY then we can't get user input, so there's
        // nothing to do except inform the user about the `-f` flag.
        if !atty::is(Stream::Stdin) {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists).into());
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

    fn prompt_accept_elv2_license(&self) -> Result<bool, InstallerError> {
        // If we're not attached to a TTY then we can't get user input, so there's
        // nothing to do except inform the user about the `--elv2-license` flag.
        if !atty::is(Stream::Stdin) {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists).into());
        }

        eprintln!("Do you accept the terms and conditions of the ELv2 license? [y/N]: ");
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

    fn extract_plugin_tarball(
        &self,
        plugin_name: &str,
        plugin_tarball_url: &str,
        client: &reqwest::blocking::Client,
    ) -> Result<Utf8PathBuf, InstallerError> {
        let download_dir = tempdir::TempDir::new(plugin_name)?;
        let download_dir_path = Utf8PathBuf::try_from(download_dir.into_path())?;
        let tarball_path = download_dir_path.join(format!("{}.tar.gz", plugin_name));
        let mut f = std::fs::File::create(&tarball_path)?;
        eprintln!("Downloading {} from {}", plugin_name, plugin_tarball_url);
        let response_bytes = client
            .get(plugin_tarball_url)
            .header(reqwest::header::USER_AGENT, "rover-client")
            .header(reqwest::header::ACCEPT, "application/octet-stream")
            .send()?
            .error_for_status()?
            .bytes()?;
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
        if fs::metadata(&path).is_err() {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("binary does not exist at `{}`", &path),
            )
            .into())
        } else {
            Ok(path)
        }
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
