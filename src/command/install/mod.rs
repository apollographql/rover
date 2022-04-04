use ansi_term::Colour::Cyan;
use camino::Utf8PathBuf;
use serde::Serialize;
use structopt::StructOpt;

use binstall::Installer;

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
    #[structopt(long, possible_values = &["supergraph-0", "supergraph-2"], case_insensitive = true)]
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
        let client = client_config.get_reqwest_client();
        let binary_name = PKG_NAME.to_string();
        let installer =
            self.get_installer(binary_name.to_string(), override_install_path.clone())?;

        if let Some(plugin) = &self.plugin {
            let plugin_name = plugin.get_name();
            let requires_elv2_license = if let Plugin::Supergraph2 = plugin {
                true
            } else {
                false
            };
            let install_location = installer
                .install_plugin(
                    &plugin_name,
                    &plugin.get_tarball_url()?,
                    requires_elv2_license,
                    self.elv2_license_accepted.unwrap_or(false),
                    &client,
                    None,
                )
                .with_context(|| format!("Could not install {}", &plugin_name))?;
            let plugin_name = format!("{}-{}", &plugin_name, &plugin.get_major());
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

            if cfg!(feature = "composition-js") && self.plugin.is_none() {
                eprintln!("installing 'rover supergraph compose' plugins... ");
                let mut plugin_installer = Install {
                    force: self.force.clone(),
                    plugin: Some(Plugin::Supergraph0),
                    elv2_license_accepted: self.elv2_license_accepted.clone(),
                };
                plugin_installer.get_versioned_plugin(
                    override_install_path.clone(),
                    client_config.clone(),
                    false,
                )?;
                plugin_installer.plugin = Some(Plugin::Supergraph2);
                plugin_installer.get_versioned_plugin(
                    override_install_path,
                    client_config.clone(),
                    false,
                )?;
                eprintln!("done installing plugins!");
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
        if let Some(plugin) = self.plugin {
            let plugin_name = plugin.get_name();
            let installer =
                self.get_installer(PKG_NAME.to_string(), override_install_path.clone())?;
            let plugin_dir = installer.get_bin_dir_path()?;
            if !skip_update {
                let latest_version = installer.get_plugin_version(&plugin.get_tarball_url()?)?;
                let plugin_name = plugin.get_name();
                let versioned_plugin = format!("{}-{}", &plugin_name, &latest_version);
                let maybe_exe = find_plugin(&plugin_dir, &versioned_plugin);
                if let Ok(exe) = maybe_exe {
                    tracing::debug!("{} exists, skipping install", &versioned_plugin);
                    Ok(exe)
                } else {
                    eprintln!(
                        "updating 'rover supergraph compose' to use {}...",
                        &versioned_plugin
                    );
                    // do the install.
                    self.run(override_install_path, client_config)?;
                    find_plugin(&plugin_dir, &versioned_plugin)
                }
            } else {
                // if we skip an update, we look in ~/.rover/bin for binaries starting with `supergraph-v`
                // and select the latest valid version from this list to use for composition.
                let mut valid_versions = Vec::new();
                std::fs::read_dir(&plugin_dir)?.for_each(|installed_plugin| {
                    if let Ok(installed_plugin) = installed_plugin {
                        if let Ok(file_type) = installed_plugin.file_type() {
                            if file_type.is_file() {
                                if let Some(file_name) = installed_plugin.file_name().to_str() {
                                    let splits: Vec<String> = file_name
                                        .to_string()
                                        .split(&format!("{}-v", plugin_name))
                                        .map(|x| x.to_string())
                                        .collect();
                                    if splits.len() == 2 {
                                        let maybe_semver = splits[1].clone();
                                        if maybe_semver.starts_with(&plugin.get_major()) {
                                            if let Ok(semver) =
                                                semver::Version::parse(&maybe_semver)
                                            {
                                                valid_versions.push(semver);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
                if valid_versions.is_empty() {
                    let mut err = RoverError::new(anyhow!(
                        "You do not have a valid {} plugin installed.",
                        plugin_name
                    ));
                    err.set_suggestion(Suggestion::Adhoc("Re-run this command without the `--skip-update` flag to install the proper plugin.".to_string()));
                    Err(err)
                } else {
                    // this sorts by semver, making the last element in the list
                    // the latest version.
                    valid_versions.sort();
                    let full_version = valid_versions.pop().unwrap();
                    let versioned_plugin = format!("{}-v{}", plugin_name, &full_version);
                    find_plugin(&plugin_dir, &versioned_plugin)
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
                binary_name: binary_name.clone(),
                force_install: self.force,
                override_install_path,
                executable_location,
            })
        } else {
            Err(anyhow!("Failed to get the current executable's path.").into())
        }
    }
}

fn find_plugin(plugin_dir: &Utf8PathBuf, versioned_plugin: &str) -> Result<Utf8PathBuf> {
    let maybe_plugin = plugin_dir.join(format!(
        "{}{}",
        versioned_plugin,
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
            err.set_suggestion(Suggestion::SubmitIssue);
        }
        Err(err)
    }
}
