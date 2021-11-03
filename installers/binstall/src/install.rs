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
    pub version: String,
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
        accept_elv2_license: bool,
    ) -> Result<Option<Utf8PathBuf>, InstallerError> {
        eprintln!("{} is licensed under the Elastic license, the full text can be found here: https://raw.githubusercontent.com/apollographql/rover/v{}/plugins/{}/LICENSE", plugin_name, &self.version, plugin_name);
        eprintln!("By installing this plugin, you accept the terms and conditions outlined by this license.");
        if !accept_elv2_license {
            self.prompt_accept_elv2_license()?;
        }
        if self.get_bin_dir_path()?.exists() {
            // The main binary already exists in a standard location
            let plugin_bin_destination = self.get_plugin_bin_path(plugin_name)?;
            if !self.force_install
                && plugin_bin_destination.exists()
                && !self.should_overwrite(&plugin_bin_destination, plugin_name)?
            {
                return Ok(None);
            }
            let plugin_bin_path = self.extract_plugin_tarball(plugin_name, plugin_tarball_url)?;
            self.write_plugin_bin_to_fs(plugin_name, &plugin_bin_path)?;
            Ok(Some(plugin_bin_destination))
        } else {
            Err(InstallerError::PluginRequiresTool {
                plugin: plugin_name.to_string(),
                tool: self.binary_name.to_string(),
            })
        }
    }

    /// Gets the location the executable will be installed to
    pub fn get_bin_dir_path(&self) -> Result<Utf8PathBuf, InstallerError> {
        let bin_dir = self.get_base_dir_path()?.join("bin");
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

    fn get_plugin_bin_path(&self, plugin_name: &str) -> Result<Utf8PathBuf, InstallerError> {
        Ok(self
            .get_bin_dir_path()?
            .join(plugin_name)
            .with_extension(env::consts::EXE_EXTENSION))
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
    ) -> Result<(), InstallerError> {
        let plugin_destination = self.get_plugin_bin_path(plugin_name)?;
        eprintln!("copying `{}` to `{}`", plugin_bin_path, &plugin_destination);
        // attempt to remove the old binary
        // but do not error if it doesn't exist.
        let _ = fs::remove_file(&plugin_destination);
        fs::copy(plugin_bin_path, &plugin_destination)?;
        // clean up temp dir
        if let Some(dist) = plugin_bin_path.parent() {
            if let Some(tempdir) = dist.parent() {
                // attempt to clean up the temp dir
                // but do not error if it doesn't exist or something goes wrong
                eprintln!("removing `{}`", tempdir);
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
    ) -> Result<Utf8PathBuf, InstallerError> {
        let download_dir = tempdir::TempDir::new(plugin_name)?;
        let download_dir_path = Utf8PathBuf::try_from(download_dir.into_path())?;
        let tarball_path = download_dir_path.join(format!("{}.tar.gz", plugin_name));
        let mut f = std::fs::File::create(&tarball_path)?;
        eprintln!("Downloading {} from {}", plugin_name, plugin_tarball_url);
        let client = reqwest::blocking::Client::new();
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
        let path = download_dir_path.join("dist").join(plugin_name);
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
