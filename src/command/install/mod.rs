use ansi_term::Colour::Cyan;
use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;

use binstall::Installer;

use crate::command::RoverOutput;
use crate::{anyhow, Context, Result};
use crate::{command::docs::shortlinks, utils::env::RoverEnvKey};
use crate::{PKG_NAME, PKG_VERSION};

use std::convert::TryFrom;
use std::env;

mod plugin;
use plugin::Plugin;

#[derive(Debug, Serialize, StructOpt)]
pub struct Install {
    /// Overwrite any existing binary without prompting for confirmation.
    #[structopt(long = "force", short = "f")]
    force: bool,

    /// Download and install an officially supported plugin from GitHub releases.
    #[structopt(long, possible_values = &["rover-fed2"], case_insensitive = true)]
    plugin: Option<Plugin>,

    /// Accept the terms and conditions of the ELv2 License without prompting for confirmation.
    #[structopt(long = "elv2-license", requires("plugin"), parse(from_str = license_accept), case_insensitive = true)]
    elv2_license_accepted: Option<bool>,
}

fn license_accept(elv2_license: &str) -> bool {
    elv2_license.to_lowercase() == "accept"
}

impl Install {
    pub fn run(&self, override_install_path: Option<Utf8PathBuf>) -> Result<RoverOutput> {
        let binary_name = PKG_NAME.to_string();
        if let Ok(executable_location) = env::current_exe() {
            let executable_location = Utf8PathBuf::try_from(executable_location)?;
            let installer = Installer {
                binary_name: binary_name.clone(),
                force_install: self.force,
                override_install_path,
                executable_location,
                version: PKG_VERSION.to_string(),
            };

            if let Some(plugin) = &self.plugin {
                let target_arch = if cfg!(target_os = "windows") {
                    Ok("x86_64-pc-windows-msvc")
                } else if cfg!(target_os = "macos") {
                    Ok("x86_64-apple-darwin")
                } else if cfg!(target_os = "linux") && !cfg!(target_env = "musl") {
                    Ok("x86_64-unknown-linux-gnu")
                } else {
                    Err(anyhow!(
                        "Your current architecture does not support installation of this plugin."
                    ))
                }?;
                let plugin_name = plugin.get_name();
                let install_location = installer
                    .install_plugin(
                        &plugin_name,
                        &plugin.get_tarball_url(target_arch),
                        self.elv2_license_accepted.unwrap_or(false),
                    )
                    .with_context(|| format!("Could not install {}", &plugin_name))?;

                if install_location.is_some() {
                    eprintln!("{} was successfully installed. Great!", &plugin_name);
                } else {
                    eprintln!("{} was not installed. To override the existing installation, you can pass the `--force` flag to the installer.", &plugin_name);
                }

                Ok(RoverOutput::EmptySuccess)
            } else {
                let install_location = installer
                    .install()
                    .with_context(|| format!("could not install {}", &binary_name))?;

                if install_location.is_some() {
                    let bin_dir_path = installer.get_bin_dir_path()?;
                    eprintln!("{} was successfully installed. Great!", &binary_name);

                    if !cfg!(windows) {
                        if let Some(path_var) = env::var_os("PATH") {
                            if !path_var
                                .to_string_lossy()
                                .to_string()
                                .contains(bin_dir_path.as_str())
                            {
                                eprintln!("\nTo get started you need Rover's bin directory ({}) in your PATH environment variable. Next time you log in this will be done automatically.", &bin_dir_path);
                                if let Ok(shell_var) = env::var("SHELL") {
                                    eprintln!(
                                        "\nTo configure your current shell, you can run:\nexec {} -l",
                                        &shell_var
                                    );
                                }
                            }
                        }
                    }

                    // these messages are duplicated in `installers/npm/install.js`
                    // for the npm installer.
                    eprintln!(
                        "If you would like to disable Rover's anonymized usage collection, you can set {}=1", RoverEnvKey::TelemetryDisabled
                    );
                    eprintln!(
                        "You can check out our documentation at {}.",
                        Cyan.normal().paint(shortlinks::get_url_from_slug("docs"))
                    );
                } else {
                    eprintln!("{} was not installed. To override the existing installation, you can pass the `--force` flag to the installer.", &binary_name);
                }
                Ok(RoverOutput::EmptySuccess)
            }
        } else {
            Err(anyhow!("Failed to get the current executable's path.").into())
        }
    }
}
