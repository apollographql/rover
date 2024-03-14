use std::{env::consts, str::FromStr};

use anyhow::{anyhow, Context};
use apollo_federation_types::config::{FederationVersion, PluginVersion, RouterVersion};
use binstall::Installer;
use camino::Utf8PathBuf;
use rover_std::{sanitize_url, Fs};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{utils::client::StudioClientConfig, RoverError, RoverErrorSuggestion, RoverResult};

// These OSX versions of the router were compiled for aarch64 only
const AARCH_OSX_ONLY_ROUTER_VERSIONS: [Version; 2] =
    [Version::new(1, 38, 0), Version::new(1, 39, 0)];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Plugin {
    Supergraph(FederationVersion),
    Router(RouterVersion),
}

impl Plugin {
    pub fn get_name(&self) -> String {
        match self {
            Self::Supergraph(_) => "supergraph".to_string(),
            Self::Router(_) => "router".to_string(),
        }
    }

    pub fn requires_elv2_license(&self) -> bool {
        match self {
            Self::Supergraph(v) => v.get_major_version() == 2,
            Self::Router(_) => true,
        }
    }

    pub fn get_tarball_version(&self) -> String {
        match self {
            Self::Supergraph(v) => v.get_tarball_version(),
            Self::Router(v) => v.get_tarball_version(),
        }
    }

    pub fn get_target_arch(&self) -> RoverResult<String> {
        self.get_arch_for_env(consts::OS, consts::ARCH)
    }

    fn get_arch_for_env(&self, os: &str, arch: &str) -> RoverResult<String> {
        let mut no_prebuilt_binaries = RoverError::new(anyhow!(
            "Your current architecture does not support installation of this plugin."
        ));
        // Sorry, no musl support for composition or the router
        if cfg!(target_env = "musl") {
            no_prebuilt_binaries.set_suggestion(RoverErrorSuggestion::CheckGnuVersion);
            return Err(no_prebuilt_binaries);
        }
        match (os, arch) {
            ("windows", _) => Ok("x86_64-pc-windows-msvc"),
            ("macos", "x86_64") => {
                match self {
                    Self::Router(RouterVersion::Exact(v)) if AARCH_OSX_ONLY_ROUTER_VERSIONS.contains(v) => {
                        // OSX router version 1.38.0 and 1.39.0 were only released on aarch64
                        Err(RoverError::new(anyhow!(
                            "Router versions {} are only available for aarch64, please use verssion 1.39.1 or above.", AARCH_OSX_ONLY_ROUTER_VERSIONS.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(" and ")
                        )))
                    },
                    _ => Ok("x86_64-apple-darwin")
                }
            } ,
            ("macos", "aarch64") => {
                match self {
                    // OSX router version starting from 1.38.0 are released for aarch64
                    Self::Router(RouterVersion::Exact(v)) if v.lt(&AARCH_OSX_ONLY_ROUTER_VERSIONS[0]) => {
                         Ok("x86_64-apple-darwin")
                    },
                    Self::Router(_) => {
                       Ok("aarch64-apple-darwin")
                   },
                   Self::Supergraph(_) =>  Ok("x86_64-apple-darwin")
                }
            } ,
            ("macos", _) => {
                match self {
                    Self::Router(RouterVersion::Exact(v)) if AARCH_OSX_ONLY_ROUTER_VERSIONS.contains(v) => {
                        // OSX router version 1.38.0 and 1.39.0 were only released on aarch64
                        Ok("aarch64-apple-darwin")
                    },
                    _ => Ok("x86_64-apple-darwin")
                }
            } ,
            ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
            ("linux", "aarch64") => {
                match self {
                    Self::Supergraph(v) => {
                        if v.supports_arm_linux() {
                            // we didn't always build aarch64 linux binaries,
                            // so check to see if this version supports them or not
                            Ok("aarch64-unknown-linux-gnu")
                        } else {
                            // if an old version doesn't have aarch64 binaries,
                            // you're out of luck
                            if v.is_fed_one() {
                                no_prebuilt_binaries.set_suggestion(RoverErrorSuggestion::Adhoc("Newer versions of this plugin have prebuilt binaries for this architecture, if you set `federation_version: 1` in your `supergraph.yaml`, it should automatically update to a supported version.".to_string()))
                            } else if v.is_fed_two() {
                                no_prebuilt_binaries.set_suggestion(RoverErrorSuggestion::Adhoc("Newer versions of this plugin have prebuilt binaries for this architecture, if you set `federation_version: 2` in your `supergraph.yaml`, it should automatically update to a supported version.".to_string()))
                            }
                            Err(no_prebuilt_binaries)
                        }
                    },
                    Self::Router(v) => {
                        match v {
                            RouterVersion::Exact(v) => {
                                if v >= &Version::new(1, 1, 0) {
                                    Ok("aarch64-unknown-linux-gnu")
                                } else {
                                    no_prebuilt_binaries.set_suggestion(RoverErrorSuggestion::Adhoc("Newer versions of this plugin have prebuilt binaries for this architecture.".to_string()));
                                    Err(no_prebuilt_binaries)
                                }
                            }
                            RouterVersion::Latest => Ok("aarch64-unknown-linux-gnu")
                        }
                    }
                }
            }
            _ => Err(no_prebuilt_binaries),
        }
        .map(|s| s.to_string())
    }

    pub fn get_tarball_url(&self) -> RoverResult<String> {
        Ok(format!(
            "{host}/tar/{name}/{target_arch}/{version}",
            host = self.get_host(),
            name = self.get_name(),
            target_arch = self.get_target_arch()?,
            version = self.get_tarball_version()
        ))
    }

    fn get_host(&self) -> String {
        std::env::var("APOLLO_ROVER_DOWNLOAD_HOST")
            .unwrap_or_else(|_| "https://rover.apollo.dev".to_string())
    }
}

impl FromStr for Plugin {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let lowercase = s.to_lowercase();
        let splits: Vec<String> = lowercase.split('@').map(|x| x.to_string()).collect();
        if splits.len() == 2 {
            let plugin_name = splits[0].clone();
            let plugin_version = splits[1].clone();
            if plugin_name == "supergraph" {
                let federation_version = FederationVersion::from_str(&plugin_version)
                    .with_context(|| {
                        format!(
                            "Invalid version '{}' for 'supergraph' plugin. Must be 'latest-0', 'latest-2', or an exact version preceeded with an '='.",
                            &plugin_version
                        )
                    })?;
                Ok(Plugin::Supergraph(federation_version))
            } else if plugin_name == "router" {
                let router_version = RouterVersion::from_str(&plugin_version).with_context({
                    || format!("Invalid version '{}' for 'router' plugin. Must be 'latest', or an exact version (>= 1.0.0 & < 2.0.0) preceeded with a 'v'", &plugin_version)
                })?;
                Ok(Plugin::Router(router_version))
            } else {
                // TODO: this should probably use ArgEnum instead
                Err(anyhow!(
                    "Invalid plugin name {}. Possible values are [supergraph, router].",
                    plugin_name
                ))
            }
        } else {
            Err(anyhow!("Plugin must be in form '{{name}}@{{version}}'."))
        }
    }
}

pub struct PluginInstaller {
    client_config: StudioClientConfig,
    rover_installer: Installer,
}

impl PluginInstaller {
    pub fn new(client_config: StudioClientConfig, rover_installer: Installer) -> Self {
        Self {
            client_config,
            rover_installer,
        }
    }

    pub async fn install(&self, plugin: &Plugin, skip_update: bool) -> RoverResult<Utf8PathBuf> {
        let skip_update_err = |plugin_name: &str, version: &str| {
            let mut err = RoverError::new(anyhow!(
                "You do not have the '{}-v{}' plugin installed.",
                plugin_name,
                version,
            ));
            if std::env::var("APOLLO_NODE_MODULES_BIN_DIR").is_ok() {
                err.set_suggestion(RoverErrorSuggestion::Adhoc(
                    "Try runnning `npm install` to reinstall the plugin.".to_string(),
                ));
            } else {
                err.set_suggestion(RoverErrorSuggestion::Adhoc(
                    "Try re-running this command without the `--skip-update` flag.".to_string(),
                ));
            }
            err
        };

        let could_not_install_plugin = |plugin_name: &str, version: &str| {
            let mut err = RoverError::new(anyhow!(
                "Could not install the '{plugin_name}-v{version}' plugin for an unknown reason."
            ));
            err.set_suggestion(RoverErrorSuggestion::SubmitIssue);
            err
        };

        let install_location = match plugin {
            Plugin::Router(version) => match version {
                RouterVersion::Exact(version) => {
                    let version = version.to_string();
                    if skip_update {
                        self.find_existing_exact(plugin, &version)?
                            .ok_or_else(|| skip_update_err(&plugin.get_name(), &version))
                    } else {
                        self.install_exact(plugin, &version)
                            .await?
                            .ok_or_else(|| could_not_install_plugin(&plugin.get_name(), &version))
                    }
                }
                RouterVersion::Latest => {
                    let major_version = 1;
                    if skip_update {
                        self.find_existing_latest_major(plugin, major_version)?
                            .ok_or_else(|| {
                                skip_update_err(
                                    &plugin.get_name(),
                                    major_version.to_string().as_str(),
                                )
                            })
                    } else {
                        self.install_latest_major(plugin).await?
                            .ok_or_else(|| {
                                could_not_install_plugin(
                                    &plugin.get_name(),
                                    major_version.to_string().as_str(),
                                )
                            })
                    }
                }
            },
            Plugin::Supergraph(version) => match version {
                FederationVersion::ExactFedOne(version)
                | FederationVersion::ExactFedTwo(version) => {
                    let version = version.to_string();
                    if skip_update {
                        self.find_existing_exact(plugin, &version)?
                            .ok_or_else(|| skip_update_err(&plugin.get_name(), &version))
                    } else {
                        self.install_exact(plugin, &version).await?
                            .ok_or_else(|| could_not_install_plugin(&plugin.get_name(), &version))
                    }
                }
                FederationVersion::LatestFedOne => {
                    let major_version = 0;
                    if skip_update {
                        self.find_existing_latest_major(plugin, major_version)?
                            .ok_or_else(|| {
                                skip_update_err(&plugin.get_name(), version.to_string().as_str())
                            })
                    } else {
                        self.install_latest_major(plugin).await?.ok_or_else(|| {
                            could_not_install_plugin(
                                &plugin.get_name(),
                                major_version.to_string().as_str(),
                            )
                        })
                    }
                }
                FederationVersion::LatestFedTwo => {
                    let major_version = 2;
                    if skip_update {
                        Ok(self
                            .find_existing_latest_major(plugin, major_version)?
                            .ok_or_else(|| {
                                skip_update_err(
                                    &plugin.get_name(),
                                    major_version.to_string().as_str(),
                                )
                            })?)
                    } else {
                        self.install_latest_major(plugin).await?.ok_or_else(|| {
                            could_not_install_plugin(
                                &plugin.get_name(),
                                major_version.to_string().as_str(),
                            )
                        })
                    }
                }
            },
        }?;

        Ok(install_location)
    }

    fn find_existing_latest_major(
        &self,
        plugin: &Plugin,
        major_version: u64,
    ) -> RoverResult<Option<Utf8PathBuf>> {
        let plugin_dir = self.rover_installer.get_bin_dir_path()?;
        let plugin_name = plugin.get_name();
        let mut installed_plugins =
            find_installed_plugins(&plugin_dir, &plugin_name, major_version)?;
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
            Ok(installed_plugins.pop())
        }
    }

    async fn install_latest_major(&self, plugin: &Plugin) -> RoverResult<Option<Utf8PathBuf>> {
        let latest_version = self
            .rover_installer
            .get_plugin_version(&plugin.get_tarball_url()?, true)?;
        if let Ok(Some(exe)) = self.find_existing_exact(plugin, &latest_version) {
            tracing::debug!("{} exists, skipping install", &exe);
            Ok(Some(exe))
        } else {
            // do the install.
            self.do_install(plugin, true).await?;
            self.find_existing_exact(plugin, &latest_version)
        }
    }

    fn find_existing_exact(
        &self,
        plugin: &Plugin,
        version: &str,
    ) -> RoverResult<Option<Utf8PathBuf>> {
        let plugin_dir = self.rover_installer.get_bin_dir_path()?;
        let plugin_name = plugin.get_name();
        Ok(find_installed_plugin(&plugin_dir, &plugin_name, version).ok())
    }

    async fn install_exact(
        &self,
        plugin: &Plugin,
        version: &str,
    ) -> RoverResult<Option<Utf8PathBuf>> {
        if let Ok(Some(exe)) = self.find_existing_exact(plugin, version) {
            Ok(Some(exe))
        } else {
            self.do_install(plugin, false).await
        }
    }

    async fn do_install(
        &self,
        plugin: &Plugin,
        is_latest: bool,
    ) -> RoverResult<Option<Utf8PathBuf>> {
        let plugin_name = plugin.get_name();
        let plugin_tarball_url = plugin.get_tarball_url()?;
        // only print the download message if the username and password have been stripped from the URL
        if let Some(sanitized_url) = sanitize_url(&plugin_tarball_url) {
            eprintln!("downloading the '{plugin_name}' plugin from {sanitized_url}");
        } else {
            eprintln!("downloading the '{plugin_name}' plugin");
        }
        Ok(self
            .rover_installer
            .install_plugin(
                &plugin_name,
                &plugin_tarball_url,
                &self.client_config.get_reqwest_client()?,
                is_latest,
            )
            .await?)
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(not(target_env = "musl"))]
    fn test_osx_plugin_versions_x64() {
        // Router versions lower than 1.38.0 or greater than or equal to 1.39.1 are available
        let available_versions = [
            Plugin::Router(RouterVersion::Latest),
            Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 1))),
            Plugin::Router(RouterVersion::Exact(Version::new(1, 37, 0))),
            // Supergraph versions remain available
            Plugin::Supergraph(FederationVersion::ExactFedTwo(Version::new(2, 7, 1))),
            Plugin::Supergraph(FederationVersion::LatestFedTwo),
        ];

        // Router version 1.38.0 and 1.39.0 are not available
        let unavailable_versions = [
            Plugin::Router(RouterVersion::Exact(Version::new(1, 38, 0))),
            Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 0))),
        ];

        for p in available_versions {
            assert_eq!(
                "x86_64-apple-darwin",
                p.get_arch_for_env("macos", "x86_64").unwrap()
            );
        }

        for p in unavailable_versions {
            p.get_arch_for_env("macos", "x86_64").unwrap_err();
        }
    }

    #[test]
    #[cfg(not(target_env = "musl"))]
    fn test_osx_plugin_versions_aarch64() {
        // Router versions lower than 1.38.0 are available under the x64 alias
        let x64_versions = [
            Plugin::Router(RouterVersion::Exact(Version::new(1, 37, 0))),
            Plugin::Router(RouterVersion::Exact(Version::new(1, 36, 0))),
            // Supergraph versions remain only x64
            Plugin::Supergraph(FederationVersion::ExactFedTwo(Version::new(2, 7, 1))),
            Plugin::Supergraph(FederationVersion::LatestFedTwo),
        ];
        // Router version 1.38.0 and above are available on their own
        let aarch_versions = [
            Plugin::Router(RouterVersion::Latest),
            Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 1))),
            Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 0))),
            Plugin::Router(RouterVersion::Exact(Version::new(1, 38, 0))),
        ];

        for p in x64_versions {
            assert_eq!(
                "x86_64-apple-darwin",
                p.get_arch_for_env("macos", "aarch64").unwrap()
            );
        }

        for p in aarch_versions {
            assert_eq!(
                "aarch64-apple-darwin",
                p.get_arch_for_env("macos", "aarch64").unwrap()
            );
        }
    }

    #[test]
    #[cfg(not(target_env = "musl"))]
    fn test_osx_plugin_versions() {
        let router_latest = Plugin::Router(RouterVersion::Latest);
        let router_exact_recent = Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 1)));
        let router_exact_one_thirty_eight =
            Plugin::Router(RouterVersion::Exact(Version::new(1, 38, 0)));
        let router_exact_one_thirty_nine =
            Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 0)));
        let router_exact_older = Plugin::Router(RouterVersion::Exact(Version::new(1, 37, 0)));

        let supergraph = Plugin::Supergraph(FederationVersion::LatestFedTwo);

        for p in [router_exact_one_thirty_eight, router_exact_one_thirty_nine] {
            assert_eq!(
                "aarch64-apple-darwin",
                p.get_arch_for_env("macos", "").unwrap()
            );
        }

        for p in [
            supergraph,
            router_latest,
            router_exact_recent,
            router_exact_older,
        ] {
            assert_eq!(
                "x86_64-apple-darwin",
                p.get_arch_for_env("macos", "").unwrap()
            );
        }
    }

    #[test]
    #[cfg(target_env = "musl")]
    fn test_plugin_version_should_fail() {
        Plugin::Router(RouterVersion::Latest)
            .get_arch_for_env("", "")
            .unwrap_err();
    }
}
