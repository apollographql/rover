use anyhow::anyhow;
use camino::Utf8PathBuf;
use clap::Parser;
use rover_std::Style;
use serde::Serialize;

use binstall::{Installer, InstallerError};

use crate::command::docs::shortlinks;
use crate::options::LicenseAccepter;
use crate::utils::{client::StudioClientConfig, env::RoverEnvKey};
use crate::{PKG_NAME, RoverError, RoverErrorSuggestion, RoverOutput, RoverResult};

use std::convert::TryFrom;
use std::env;

mod plugin;
pub(crate) use plugin::McpServerVersion;
pub(crate) use plugin::{Plugin, PluginInstaller};

#[derive(Debug, Serialize, Parser)]
pub struct Install {
    /// Overwrite any existing binary without prompting for confirmation.
    #[arg(long = "force", short = 'f')]
    pub(crate) force: bool,

    /// Download and install an officially supported plugin from GitHub releases.
    #[arg(long)]
    pub(crate) plugin: Option<Plugin>,

    #[clap(flatten)]
    pub(crate) elv2_license_accepter: LicenseAccepter,
}

impl Install {
    pub async fn do_install(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        let binary_name = PKG_NAME.to_string();
        let rover_installer = self.get_installer(binary_name.to_string(), override_install_path)?;

        if let Some(plugin) = &self.plugin {
            let requires_elv2_license = plugin.requires_elv2_license();
            if requires_elv2_license {
                self.elv2_license_accepter
                    .require_elv2_license(&client_config)?;
            }
            let plugin_installer = PluginInstaller::new(client_config, rover_installer, self.force);
            plugin_installer.install(plugin, false).await?;

            Ok(RoverOutput::EmptySuccess)
        } else {
            // install rover
            let install_location = rover_installer
                .install().map_err(|e| {
                    let mut err = RoverError::from(anyhow!("Could not install '{binary_name}' because {}", e.to_string().to_lowercase()));
                    if matches!(e, InstallerError::NoTty) {
                        err.set_suggestion(RoverErrorSuggestion::Adhoc("Try re-running this command with the `--force` flag to overwrite the existing binary.".to_string()));
                    }
                    err
                })?;

            if install_location.is_some() {
                let bin_dir_path = rover_installer.get_bin_dir_path()?;
                eprintln!("{} was successfully installed. Great!", &binary_name);

                if !cfg!(windows)
                    && let Some(path_var) = env::var_os("PATH")
                    && !path_var
                        .to_string_lossy()
                        .to_string()
                        .contains(bin_dir_path.as_str())
                {
                    eprintln!(
                        "\nTo get started you need Rover's bin directory ({}) in your PATH environment variable. Next time you log in this will be done automatically.",
                        &bin_dir_path
                    );
                    if let Ok(shell_var) = env::var("SHELL") {
                        eprintln!(
                            "\nTo configure your current shell, you can run:\nexec {} -l",
                            &shell_var
                        );
                    }
                }

                // these messages are duplicated in `installers/npm/binary.js`
                // for the npm installer.
                eprintln!(
                    "If you would like to disable Rover's anonymized usage collection, you can set {}=true",
                    RoverEnvKey::TelemetryDisabled
                );
                eprintln!(
                    "You can check out our documentation at {}.",
                    Style::Link.paint(shortlinks::get_url_from_slug("docs"))
                );
            } else {
                eprintln!(
                    "{} was not installed. To override the existing installation, you can pass the `--force` flag to the installer.",
                    &binary_name
                );
            }

            Ok(RoverOutput::EmptySuccess)
        }
    }

    #[cfg(feature = "composition-js")]
    pub(crate) async fn get_versioned_plugin(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        skip_update: bool,
    ) -> RoverResult<Utf8PathBuf> {
        let rover_installer = self.get_installer(PKG_NAME.to_string(), override_install_path)?;
        if let Some(plugin) = &self.plugin {
            let plugin_installer = PluginInstaller::new(client_config, rover_installer, self.force);
            plugin_installer.install(plugin, skip_update).await
        } else {
            let mut err =
                RoverError::new(anyhow!("Could not find a plugin to get a version from."));
            err.set_suggestion(RoverErrorSuggestion::SubmitIssue);
            Err(err)
        }
    }

    fn get_installer(
        &self,
        binary_name: String,
        override_install_path: Option<Utf8PathBuf>,
    ) -> RoverResult<Installer> {
        if let Ok(executable_location) = env::current_exe() {
            let executable_location = Utf8PathBuf::try_from(executable_location)?;
            Ok(Installer {
                binary_name,
                force_install: self.force,
                override_install_path,
                executable_location,
            })
        } else {
            Err(anyhow!("Failed to get the current executable's path.").into())
        }
    }
}
