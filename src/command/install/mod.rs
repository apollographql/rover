use anyhow::{anyhow, Context};
use camino::Utf8PathBuf;
use clap::Parser;
use rover_std::Style;
use serde::Serialize;

use binstall::Installer;

use crate::options::LicenseAccepter;
use crate::utils::client::StudioClientConfig;
use crate::{command::docs::shortlinks, utils::env::RoverEnvKey};
use crate::{RoverOutput, RoverResult, PKG_NAME};

use std::convert::TryFrom;
use std::env;

#[cfg(feature = "composition-js")]
use apollo_federation_types::config::PluginVersion;

mod plugin;
pub(crate) use plugin::Plugin;

#[cfg(feature = "composition-js")]
use apollo_federation_types::config::{FederationVersion, RouterVersion};

#[cfg(feature = "composition-js")]
use rover_std::Fs;

#[cfg(feature = "composition-js")]
use crate::{RoverError, RoverErrorSuggestion};

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
    pub fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_reqwest_client()?;
        let binary_name = PKG_NAME.to_string();
        let installer = self.get_installer(binary_name.to_string(), override_install_path)?;

        if let Some(plugin) = &self.plugin {
            let plugin_name = plugin.get_name();
            let requires_elv2_license = plugin.requires_elv2_license();
            if requires_elv2_license {
                self.elv2_license_accepter
                    .require_elv2_license(&client_config)?;
            }
            let install_location = installer.install_plugin(
                &plugin_name,
                &plugin.get_tarball_url()?,
                &client,
                None,
            )?;
            let plugin_name = format!("{}-{}", &plugin_name, &plugin.get_tarball_version());
            if let Some(install_location) = install_location {
                eprintln!(
                    "{} was successfully installed to {}. Great!",
                    &plugin_name, &install_location
                );
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
                    Style::Link.paint(&shortlinks::get_url_from_slug("docs"))
                );
            } else {
                eprintln!("{} was not installed. To override the existing installation, you can pass the `--force` flag to the installer.", &binary_name);
            }

            Ok(RoverOutput::EmptySuccess)
        }
    }

    #[cfg(feature = "composition-js")]
    pub(crate) fn get_versioned_plugin(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        skip_update: bool,
    ) -> RoverResult<Utf8PathBuf> {
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
                                    err.set_suggestion(RoverErrorSuggestion::Adhoc(
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
                                err.set_suggestion(RoverErrorSuggestion::Adhoc(
                                        "Re-run this command without the `--skip-update` flag to install the proper plugin."
                                            .to_string(),
                                    ));
                                Err(err)
                            }
                        }
                    }
                }
                Plugin::Router(plugin_version) => {
                    let plugin_name = "router";
                    let plugin_version = match plugin_version {
                        RouterVersion::Exact(v) => v.to_string(),
                        _ => {
                            return Err(RoverError::new(anyhow!(
                            "the 'router' plugin does not yet support pulling the latest version."
                        )))
                        }
                    };
                    let maybe_exe =
                        find_installed_plugin(&plugin_dir, plugin_name, &plugin_version);
                    if let Ok(exe) = maybe_exe {
                        tracing::debug!("{} exists, skipping install", &exe);
                        Ok(exe)
                    } else {
                        eprintln!(
                            "installing plugin '{}-{}' for 'rover dev'...",
                            &plugin_name, &plugin_version
                        );
                        // do the install.
                        self.run(override_install_path, client_config)?;
                        find_installed_plugin(&plugin_dir, plugin_name, &plugin_version)
                    }
                }
            }
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

#[cfg(feature = "composition-js")]
fn find_installed_plugins(
    plugin_dir: &Utf8PathBuf,
    plugin_name: &str,
    major_version: u64,
) -> RoverResult<Vec<Utf8PathBuf>> {
    // if we skip an update, we look in ~/.rover/bin for binaries starting with `supergraph-v`
    // and select the latest valid version from this list to use for composition.
    let mut installed_versions = Vec::new();
    Fs::get_dir_entries(plugin_dir)?.for_each(|installed_plugin| {
        if let Ok(installed_plugin) = installed_plugin {
            if let Ok(file_type) = installed_plugin.file_type() {
                if file_type.is_file() {
                    let splits: Vec<String> = installed_plugin
                        .file_name()
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

#[cfg(feature = "composition-js")]
fn find_installed_plugin(
    plugin_dir: &Utf8PathBuf,
    plugin_name: &str,
    version: &str,
) -> RoverResult<Utf8PathBuf> {
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
    if Fs::assert_path_exists(&maybe_plugin).is_ok() {
        Ok(maybe_plugin)
    } else {
        let mut err = RoverError::new(anyhow!("Could not find plugin at {}", &maybe_plugin));
        if std::env::var("APOLLO_NODE_MODULES_BIN_DIR").is_ok() {
            err.set_suggestion(RoverErrorSuggestion::Adhoc(
                "Try runnning `npm install` to reinstall the plugin.".to_string(),
            ));
        } else {
            err.set_suggestion(RoverErrorSuggestion::Adhoc(
                "Try re-running this command without the `--skip-update` flag.".to_string(),
            ));
        }
        Err(err)
    }
}
