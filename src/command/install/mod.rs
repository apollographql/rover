use ansi_term::Colour::Cyan;
use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;

use binstall::{Installer, InstallerError};

use crate::command::RoverOutput;
use crate::utils::client::StudioClientConfig;
use crate::PKG_NAME;
use crate::{anyhow, error::RoverError, Context, Result, Suggestion};
use crate::{command::docs::shortlinks, utils::env::RoverEnvKey};

use std::convert::TryFrom;
use std::env;

mod plugin;
pub(crate) use plugin::Plugin;

#[derive(Debug, Serialize, StructOpt)]
pub struct Install {
    /// Overwrite any existing binary without prompting for confirmation.
    #[structopt(long = "force", short = "f")]
    pub(crate) force: bool,

    /// Download and install an officially supported plugin from GitHub releases.
    #[structopt(long, case_insensitive = true)]
    pub(crate) plugin: Option<Plugin>,

    /// Accept the terms and conditions of the ELv2 License without prompting for confirmation.
    #[structopt(long = "elv2-license", parse(from_str = license_accept), case_insensitive = true, env = "APOLLO_ELV2_LICENSE")]
    pub(crate) elv2_license_accepted: Option<bool>,
}

pub(crate) fn license_accept(elv2_license: &str) -> bool {
    elv2_license.to_lowercase() == "accept"
}

impl Install {
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> Result<RoverOutput> {
        let accept_elv2_license = if let Some(elv2_license_accepted) = self.elv2_license_accepted {
            if elv2_license_accepted {
                client_config.config.accept_elv2_license()?;
                true
            } else {
                false
            }
        } else {
            client_config.config.did_accept_elv2_license()
        };

        let client = client_config.get_reqwest_client();
        let binary_name = PKG_NAME.to_string();
        let installer = self.get_installer(binary_name.to_string(), override_install_path)?;

        if let Some(plugin) = &self.plugin {
            let plugin_name = plugin.get_name();
            let requires_elv2_license = plugin.requires_elv2_license();
            let install_location = installer
                .install_plugin(
                    &plugin_name,
                    &plugin.get_tarball_url()?,
                    requires_elv2_license,
                    accept_elv2_license,
                    &client,
                    None,
                )
                .map_err(|e| {
                    if matches!(&e, InstallerError::MustAcceptElv2 { .. }) {
                        let mut err = RoverError::new(e);
                        let mut suggestion = "Before running this command again, you need to either set `APOLLO_ELV2_LICENSE=accept` as an environment variable, or pass the `--elv2-license=accept` argument.".to_string();
                        if std::env::var_os("CI").is_none() {
                            suggestion.push_str(" You will only need to do this once on this machine.")
                        }
                        err.set_suggestion(Suggestion::Adhoc(suggestion));
                        err
                    } else {
                        RoverError::new(e)
                    }
                })?;
            if requires_elv2_license && !accept_elv2_license {
                // we made it past the install, which means they accepted the y/N prompt
                client_config.config.accept_elv2_license()?;
            }
            let plugin_name = format!("{}-{}", &plugin_name, &plugin.get_major_version());
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
    }

    pub(crate) fn get_versioned_plugin(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        skip_update: bool,
    ) -> Result<Utf8PathBuf> {
        let installer = self.get_installer(PKG_NAME.to_string(), override_install_path.clone())?;
        let plugin_dir = installer.get_bin_dir_path()?;
        if let Some(plugin) = &self.plugin {
            let plugin_name = plugin.get_name();
            match plugin {
                Plugin::Supergraph(federation_version) => {
                    match federation_version {
                        FederationVersion::LatestFedOne | FederationVersion::LatestFedTwo => {
                            if skip_update {
                                let mut installed_plugins = find_installed_plugins(
                                    &plugin_dir,
                                    &plugin_name,
                                    federation_version.get_major_version(),
                                )?;
                                if installed_plugins.is_empty() {
                                    let mut err = RoverError::new(anyhow!(
                                        "You do not have any '{}' plugins installed in '{}'.",
                                        &plugin_name,
                                        &plugin_dir
                                    ));
                                    err.set_suggestion(Suggestion::Adhoc(
                                        "Re-run this command without the `--skip-update` flag to install the proper plugin."
                                            .to_string(),
                                    ));
                                    Err(err)
                                } else {
                                    // installed_plugins are sorted by semver
                                    // this pop will take the latest valid installed version
                                    Ok(installed_plugins.pop().unwrap())
                                }
                            } else {
                                let latest_version =
                                    installer.get_plugin_version(&plugin.get_tarball_url()?)?;
                                let maybe_exe = find_installed_plugin(
                                    &plugin_dir,
                                    &plugin_name,
                                    &latest_version,
                                );
                                if let Ok(exe) = maybe_exe {
                                    tracing::debug!("{} exists, skipping install", &exe);
                                    Ok(exe)
                                } else {
                                    eprintln!(
                                        "installing plugin '{}-{}' for 'rover supergraph compose'...",
                                        &plugin_name, &federation_version
                                    );
                                    // do the install.
                                    self.run(override_install_path, client_config)?;
                                    find_installed_plugin(
                                        &plugin_dir,
                                        &plugin_name,
                                        &latest_version,
                                    )
                                }
                            }
                        }
                        FederationVersion::ExactFedOne(version)
                        | FederationVersion::ExactFedTwo(version) => {
                            let maybe_exe = find_installed_plugin(
                                &plugin_dir,
                                &plugin_name,
                                &version.to_string(),
                            );
                            if let Ok(exe) = maybe_exe {
                                tracing::debug!("{} exists, skipping install", &exe);
                                Ok(exe)
                            } else if !skip_update {
                                eprintln!(
                                    "installing plugin '{}-v{}' for 'rover supergraph compose'...",
                                    &plugin_name, version
                                );
                                // do the install.
                                self.run(override_install_path, client_config)?;
                                find_installed_plugin(
                                    &plugin_dir,
                                    &plugin_name,
                                    &version.to_string(),
                                )
                            } else {
                                let mut err = RoverError::new(anyhow!(
                                    "You do not have '{}-v{}' installed in '{}'.",
                                    &plugin_name,
                                    version,
                                    &plugin_dir
                                ));
                                err.set_suggestion(Suggestion::Adhoc(
                                        "Re-run this command without the `--skip-update` flag to install the proper plugin."
                                            .to_string(),
                                    ));
                                Err(err)
                            }
                        }
                    }
                }
            }
        } else {
            let mut err =
                RoverError::new(anyhow!("Could not find a plugin to get a version from."));
            err.set_suggestion(Suggestion::SubmitIssue);
            Err(err)
        }
    }

    fn get_installer(
        &self,
        binary_name: String,
        override_install_path: Option<Utf8PathBuf>,
    ) -> Result<Installer> {
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

fn find_installed_plugins(
    plugin_dir: &Utf8PathBuf,
    plugin_name: &str,
    major_version: u64,
) -> Result<Vec<Utf8PathBuf>> {
    // if we skip an update, we look in ~/.rover/bin for binaries starting with `supergraph-v`
    // and select the latest valid version from this list to use for composition.
    let mut installed_versions = Vec::new();
    std::fs::read_dir(plugin_dir)?.for_each(|installed_plugin| {
        if let Ok(installed_plugin) = installed_plugin {
            if let Ok(file_type) = installed_plugin.file_type() {
                if file_type.is_file() {
                    if let Some(file_name) = installed_plugin.file_name().to_str() {
                        let splits: Vec<String> = file_name
                            .to_string()
                            .split("-v")
                            .map(|x| x.to_string())
                            .collect();
                        if splits.len() == 2 && splits[0] == plugin_name {
                            let maybe_semver = splits[1].clone();
                            if let Ok(semver) = semver::Version::parse(&maybe_semver) {
                                if semver.major == major_version {
                                    installed_versions.push(semver);
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    // this sorts by semver, making the last element in the list
    // the latest version.
    installed_versions.sort();
    let installed_plugins = installed_versions
        .iter()
        .map(|v| format!("{}-v{}{}", plugin_name, v, std::env::consts::EXE_SUFFIX).into())
        .collect();
    Ok(installed_plugins)
}

fn find_installed_plugin(
    plugin_dir: &Utf8PathBuf,
    plugin_name: &str,
    version: &str,
) -> Result<Utf8PathBuf> {
    let version = if let Some(version) = version.strip_prefix('v') {
        version.to_string()
    } else {
        version.to_string()
    };
    let maybe_plugin = plugin_dir.join(format!(
        "{}-v{}{}",
        plugin_name,
        version,
        std::env::consts::EXE_SUFFIX
    ));
    if std::fs::metadata(&maybe_plugin).is_ok() {
        Ok(maybe_plugin)
    } else {
        let mut err = RoverError::new(anyhow!("Could not find plugin at {}", &maybe_plugin));
        if std::env::var("APOLLO_NODE_MODULES_BIN_DIR").is_ok() {
            err.set_suggestion(Suggestion::Adhoc(
                "Try runnning `npm install` to reinstall the plugin.".to_string(),
            ));
        } else {
            err.set_suggestion(Suggestion::Adhoc(
                "Try re-running this command without the `--skip-update` flag.".to_string(),
            ));
        }
        Err(err)
    }
}
