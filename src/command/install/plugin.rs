use std::{env::consts, str::FromStr};

use anyhow::{Context, anyhow};
use apollo_federation_types::config::{FederationVersion, PluginVersion, RouterVersion};
use binstall::Installer;
use camino::Utf8PathBuf;
use rover_std::{Fs, sanitize_url};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{RoverError, RoverErrorSuggestion, RoverResult, utils::client::StudioClientConfig};

mod error;
mod mcp;

pub(crate) use mcp::Version as McpServerVersion;

// These OSX versions of the router were compiled for aarch64 only
const AARCH_OSX_ONLY_ROUTER_VERSIONS: [Version; 2] =
    [Version::new(1, 38, 0), Version::new(1, 39, 0)];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Plugin {
    Supergraph(FederationVersion),
    Router(RouterVersion),
    McpServer(mcp::Version),
}

impl Plugin {
    pub fn get_name(&self) -> String {
        match self {
            Self::Supergraph(_) => "supergraph".to_string(),
            Self::Router(_) => "router".to_string(),
            Self::McpServer(_) => "apollo-mcp-server".to_string(),
        }
    }

    pub fn requires_elv2_license(&self) -> bool {
        match self {
            Self::Supergraph(v) => v.get_major_version() == 2,
            Self::Router(_) => true,
            Self::McpServer(_) => true,
        }
    }

    pub fn get_tarball_version(&self) -> String {
        match self {
            Self::Supergraph(v) => v.get_tarball_version(),
            Self::Router(v) => v.get_tarball_version(),
            Self::McpServer(v) => v.get_tarball_version(),
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
                    Self::Router(_) | Self::McpServer(_) => {
                       Ok("aarch64-apple-darwin")
                   },
                   Self::Supergraph(v) => {
                       if v.supports_arm_macos() {
                           // we didn't always build aarch64 binaries,
                           // so check to see if this version supports them or not
                           Ok("aarch64-apple-darwin")
                       } else {
                           Ok("x86_64-apple-darwin")
                       }
                   }
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
                    }
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
                            RouterVersion::LatestOne | RouterVersion::LatestTwo => Ok("aarch64-unknown-linux-gnu")
                        }
                    }
                    Self::McpServer(_) => Ok("aarch64-unknown-linux-gnu"),
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
                            "Invalid version '{}' for 'supergraph' plugin. Must be 'latest-0', 'latest-2', or an exact version preceded with an '='.",
                            &plugin_version
                        )
                    })?;
                Ok(Plugin::Supergraph(federation_version))
            } else if plugin_name == "router" {
                let router_version = RouterVersion::from_str(&plugin_version).with_context({
                    || format!("Invalid version '{}' for 'router' plugin. Must be 'latest', or an exact version (>= 1.0.0 & < 2.0.0) preceded with a 'v'", &plugin_version)
                })?;
                Ok(Plugin::Router(router_version))
            } else if plugin_name == "apollo-mcp-server" {
                let mcp_version = mcp::Version::from_str(&plugin_version).with_context({
                    || format!("Invalid version '{}' for 'apollo-mcp-server' plugin. Must be 'latest' or an exact version preceded with a 'v'", &plugin_version)
                })?;
                Ok(Plugin::McpServer(mcp_version))
            } else {
                // TODO: this should probably use ArgEnum instead
                Err(anyhow!(
                    "Invalid plugin name {}. Possible values are [apollo-mcp-server, supergraph, router].",
                    plugin_name
                ))
            }
        } else {
            Err(anyhow!("Plugin must be in form '{{name}}@{{version}}'."))
        }
    }
}

/// Installer for plugins such as the supergraph binary
pub struct PluginInstaller {
    /// StudioClientConfig for Studio and GraphQL client
    client_config: StudioClientConfig,
    /// The installer that fetches and installs the plugin
    installer: Installer,
    /// Whether to overwrite the plugin if it already exists
    force: bool,
}

fn skip_update_error(plugin_name: &str, version: &str) -> RoverError {
    let mut err = RoverError::new(anyhow!(
        "You do not have the '{}-v{}' plugin installed.",
        plugin_name,
        version,
    ));
    if std::env::var("APOLLO_NODE_MODULES_BIN_DIR").is_ok() {
        err.set_suggestion(RoverErrorSuggestion::Adhoc(
            "Try running `npm install` to reinstall the plugin.".to_string(),
        ));
    } else {
        err.set_suggestion(RoverErrorSuggestion::Adhoc(
            "Try re-running this command without the `--skip-update` flag.".to_string(),
        ));
    }
    err
}

fn could_not_install_plugin(plugin_name: &str, version: &str) -> RoverError {
    let mut err = RoverError::new(anyhow!(
        "Could not install the '{plugin_name}-v{version}' plugin for an unknown reason."
    ));
    err.set_suggestion(RoverErrorSuggestion::SubmitIssue);
    err
}

impl PluginInstaller {
    pub const fn new(client_config: StudioClientConfig, installer: Installer, force: bool) -> Self {
        Self {
            client_config,
            installer,
            force,
        }
    }

    pub async fn install(&self, plugin: &Plugin, skip_update: bool) -> RoverResult<Utf8PathBuf> {
        let install_location = match plugin {
            Plugin::Router(version) => match version {
                RouterVersion::Exact(version) => {
                    let version = version.to_string();
                    self.find_or_install_exact(plugin, &version, skip_update)
                        .await
                }
                RouterVersion::LatestOne => {
                    let major_version = 1;
                    self.find_or_install_latest_major(plugin, major_version, skip_update)
                        .await
                }
                RouterVersion::LatestTwo => {
                    let major_version = 2;
                    self.find_or_install_latest_major(plugin, major_version, skip_update)
                        .await
                }
            },
            Plugin::Supergraph(version) => match version {
                FederationVersion::ExactFedOne(version)
                | FederationVersion::ExactFedTwo(version) => {
                    let version = version.to_string();
                    self.find_or_install_exact(plugin, &version, skip_update)
                        .await
                }
                FederationVersion::LatestFedOne => {
                    let major_version = 0;
                    self.find_or_install_latest_major(plugin, major_version, skip_update)
                        .await
                }
                FederationVersion::LatestFedTwo => {
                    let major_version = 2;
                    self.find_or_install_latest_major(plugin, major_version, skip_update)
                        .await
                }
            },
            Plugin::McpServer(version) => match version {
                mcp::Version::Exact(version) => {
                    let version = version.to_string();
                    self.find_or_install_exact(plugin, &version, skip_update)
                        .await
                }
                mcp::Version::Latest => {
                    let major_version = 0;
                    self.find_or_install_latest_major(plugin, major_version, skip_update)
                        .await
                }
            },
        }?;

        Ok(install_location)
    }

    async fn find_or_install_exact(
        &self,
        plugin: &Plugin,
        version: &str,
        skip_update: bool,
    ) -> RoverResult<Utf8PathBuf> {
        if skip_update {
            self.find_existing_exact(plugin, version)?
                .ok_or_else(|| skip_update_error(&plugin.get_name(), version))
        } else {
            self.install_exact(plugin, version)
                .await?
                .ok_or_else(|| could_not_install_plugin(&plugin.get_name(), version))
        }
    }

    async fn find_or_install_latest_major(
        &self,
        plugin: &Plugin,
        major_version: u64,
        skip_update: bool,
    ) -> RoverResult<Utf8PathBuf> {
        if skip_update {
            self.find_existing_latest_major(plugin, major_version)?
                .ok_or_else(|| skip_update_error(&plugin.get_name(), &major_version.to_string()))
        } else {
            self.install_latest_major(plugin).await?.ok_or_else(|| {
                could_not_install_plugin(&plugin.get_name(), &major_version.to_string())
            })
        }
    }

    fn find_existing_latest_major(
        &self,
        plugin: &Plugin,
        major_version: u64,
    ) -> RoverResult<Option<Utf8PathBuf>> {
        let plugin_dir = self.installer.get_bin_dir_path()?;
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
            .installer
            .get_plugin_version(&plugin.get_tarball_url()?, true)
            .await?;

        if let Ok(Some(exe)) = self.find_existing_exact(plugin, &latest_version)
            && !self.force
        {
            tracing::debug!("{} exists, skipping install", &exe);
            return Ok(Some(exe));
        }
        // do the install.
        self.do_install(plugin, true).await?;
        self.find_existing_exact(plugin, &latest_version)
    }

    fn find_existing_exact(
        &self,
        plugin: &Plugin,
        version: &str,
    ) -> RoverResult<Option<Utf8PathBuf>> {
        let plugin_dir = self.installer.get_bin_dir_path()?;
        let plugin_name = plugin.get_name();
        Ok(find_installed_plugin(&plugin_dir, &plugin_name, version).ok())
    }

    async fn install_exact(
        &self,
        plugin: &Plugin,
        version: &str,
    ) -> RoverResult<Option<Utf8PathBuf>> {
        if let Ok(Some(exe)) = self.find_existing_exact(plugin, version)
            && !self.force
        {
            tracing::debug!("{} exists, skipping install", &exe);
            return Ok(Some(exe));
        }
        self.do_install(plugin, false).await
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
            .installer
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
        if let Ok(installed_plugin) = installed_plugin
            && let Ok(file_type) = installed_plugin.file_type()
            && file_type.is_file()
        {
            let splits: Vec<String> = installed_plugin
                .file_name()
                .split("-v")
                .map(|x| x.to_string())
                .collect();
            if splits.len() == 2 && splits[0] == plugin_name {
                let maybe_semver = splits[1].clone();
                if let Ok(semver) = semver::Version::parse(&maybe_semver)
                    && semver.major == major_version
                {
                    installed_versions.push(semver);
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
                "Try running `npm install` to reinstall the plugin.".to_string(),
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
    use rstest::rstest;
    use speculoos::{assert_that, prelude::ResultAssertions};

    use super::*;

    #[rstest]
    // #### macOS, x86_64 ####
    // # Router #
    #[case::macos_x86_64_router_latest_one(
        Plugin::Router(RouterVersion::LatestOne),
        "macos",
        "x86_64",
        Some("x86_64-apple-darwin")
    )]
    #[case::macos_x86_64_router_latest_two(
        Plugin::Router(RouterVersion::LatestTwo),
        "macos",
        "x86_64",
        Some("x86_64-apple-darwin")
    )]
    #[case::macos_x86_64_router_v_1_39_1(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 1))),
        "macos",
        "x86_64",
        Some("x86_64-apple-darwin")
    )]
    #[case::macos_x86_64_router_v_1_37_0(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 37, 0))),
        "macos",
        "x86_64",
        Some("x86_64-apple-darwin")
    )]
    // Router v1.38.0, and v1.39.0 were never released from x86 macOS
    #[case::macos_x86_64_router_v_1_39_0_fail(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 0))),
        "macos",
        "x86_64",
        None
    )]
    #[case::macos_x86_64_router_v_1_38_0_fail(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 38, 0))),
        "macos",
        "x86_64",
        None
    )]
    // # Supergraph #
    #[case::macos_x86_64_supergraph_latest(
        Plugin::Supergraph(FederationVersion::LatestFedTwo),
        "macos",
        "x86_64",
        Some("x86_64-apple-darwin")
    )]
    #[case::macos_x86_64_supergraph_v_2_7_1(
        Plugin::Supergraph(FederationVersion::ExactFedTwo(Version::new(2, 7, 1))),
        "macos",
        "x86_64",
        Some("x86_64-apple-darwin")
    )]
    // ### macOS, aarch64 ###
    // # Router #
    #[case::macos_aarch64_router_latest_one(
        Plugin::Router(RouterVersion::LatestOne),
        "macos",
        "aarch64",
        Some("aarch64-apple-darwin")
    )]
    #[case::macos_aarch64_router_latest_two(
        Plugin::Router(RouterVersion::LatestTwo),
        "macos",
        "aarch64",
        Some("aarch64-apple-darwin")
    )]
    #[case::macos_aarch64_router_v_1_39_1(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 1))),
        "macos",
        "aarch64",
        Some("aarch64-apple-darwin")
    )]
    #[case::macos_aarch64_router_v_1_39_0(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 0))),
        "macos",
        "aarch64",
        Some("aarch64-apple-darwin")
    )]
    #[case::macos_aarch64_router_v_1_38_0(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 38, 0))),
        "macos",
        "aarch64",
        Some("aarch64-apple-darwin")
    )]
    // Router v1.37.0 and below should still get the x86_64 binary as the aarch64 doesn't exist
    #[case::macos_aarch64_router_v_1_37_0(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 37, 0))),
        "macos",
        "aarch64",
        Some("x86_64-apple-darwin")
    )]
    #[case::macos_aarch64_router_v_1_36_0(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 36, 0))),
        "macos",
        "aarch64",
        Some("x86_64-apple-darwin")
    )]
    // # Supergraph #
    #[case::macos_aarch64_supergraph_latest_fed2(
        Plugin::Supergraph(FederationVersion::LatestFedTwo),
        "macos",
        "aarch64",
        Some("aarch64-apple-darwin")
    )]
    // v2.7.3 is first version to support aarch64 for macOS, to maintain previous behaviour
    // we get x86_64 back if we ask for older versions.
    #[case::macos_aarch64_supergraph_v_2_7_4(
        Plugin::Supergraph(FederationVersion::ExactFedTwo(Version::new(2, 7, 4))),
        "macos",
        "aarch64",
        Some("aarch64-apple-darwin")
    )]
    #[case::macos_aarch64_supergraph_v_2_6_1_fail(
        Plugin::Supergraph(FederationVersion::ExactFedTwo(Version::new(2, 6, 1))),
        "macos",
        "aarch64",
        Some("x86_64-apple-darwin")
    )]
    // There are no Federation 1 versions that support aarch64
    #[case::macos_aarch64_supergraph_latest_fed1(
        Plugin::Supergraph(FederationVersion::LatestFedOne),
        "macos",
        "aarch64",
        Some("x86_64-apple-darwin")
    )]
    // ### macOS, "" ###
    // # Router #
    #[case::macos_empty_router_latest_one(
        Plugin::Router(RouterVersion::LatestOne),
        "macos",
        "",
        Some("x86_64-apple-darwin")
    )]
    #[case::macos_empty_router_latest_two(
        Plugin::Router(RouterVersion::LatestTwo),
        "macos",
        "",
        Some("x86_64-apple-darwin")
    )]
    #[case::macos_empty_router_v_1_39_1(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 1))),
        "macos",
        "",
        Some("x86_64-apple-darwin")
    )]
    // Since v1.38.0 and v1.39.0 were never released for x86_64 we have to default to the aarch64 versions here
    #[case::macos_empty_router_v_1_39_0(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 0))),
        "macos",
        "",
        Some("aarch64-apple-darwin")
    )]
    #[case::macos_empty_router_v_1_38_0(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 38, 0))),
        "macos",
        "",
        Some("aarch64-apple-darwin")
    )]
    #[case::macos_empty_router_v_1_37_0(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 37, 0))),
        "macos",
        "",
        Some("x86_64-apple-darwin")
    )]
    // # Supergraph
    #[case::macos_empty_supergraph_latest(
        Plugin::Supergraph(FederationVersion::LatestFedTwo),
        "macos",
        "",
        Some("x86_64-apple-darwin")
    )]
    // ### Windows, "" ###
    // # Router #
    #[case::windows_empty_router_latest_one(
        Plugin::Router(RouterVersion::LatestOne),
        "windows",
        "",
        Some("x86_64-pc-windows-msvc")
    )]
    #[case::windows_empty_router_latest_two(
        Plugin::Router(RouterVersion::LatestTwo),
        "windows",
        "",
        Some("x86_64-pc-windows-msvc")
    )]
    // # Supergraph #
    #[case::windows_empty_supergraph_latest(
        Plugin::Supergraph(FederationVersion::LatestFedTwo),
        "windows",
        "",
        Some("x86_64-pc-windows-msvc")
    )]
    // ### Linux, x86_64 ###
    // # Router #
    #[case::linux_x86_64_router_latest_one(
        Plugin::Router(RouterVersion::LatestOne),
        "linux",
        "x86_64",
        Some("x86_64-unknown-linux-gnu")
    )]
    #[case::linux_x86_64_router_latest_two(
        Plugin::Router(RouterVersion::LatestTwo),
        "linux",
        "x86_64",
        Some("x86_64-unknown-linux-gnu")
    )]
    // # Supergraph #
    #[case::linux_x86_64_supergraph_latest(
        Plugin::Supergraph(FederationVersion::LatestFedTwo),
        "linux",
        "x86_64",
        Some("x86_64-unknown-linux-gnu")
    )]
    // ### Linux, aarch64 ###
    // # Router #
    #[case::linux_aarch64_router_latest_one(
        Plugin::Router(RouterVersion::LatestOne),
        "linux",
        "aarch64",
        Some("aarch64-unknown-linux-gnu")
    )]
    #[case::linux_aarch64_router_latest_two(
        Plugin::Router(RouterVersion::LatestTwo),
        "linux",
        "aarch64",
        Some("aarch64-unknown-linux-gnu")
    )]
    #[case::linux_aarch64_router_v_1_39_0(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 39, 0))),
        "linux",
        "aarch64",
        Some("aarch64-unknown-linux-gnu")
    )]
    // Router supports ARM on Linux from 1.1.0 and above
    #[case::linux_aarch64_router_v_1_0_25_fail(
        Plugin::Router(RouterVersion::Exact(Version::new(1, 0, 25))),
        "linux",
        "aarch64",
        None
    )]
    // # Supergraph #
    #[case::linux_aarch64_supergraph_latest_fed2(
        Plugin::Supergraph(FederationVersion::LatestFedTwo),
        "linux",
        "aarch64",
        Some("aarch64-unknown-linux-gnu")
    )]
    #[case::linux_aarch64_supergraph_v_2_3_5(
        Plugin::Supergraph(FederationVersion::ExactFedTwo(Version::new(2, 3, 5))),
        "linux",
        "aarch64",
        Some("aarch64-unknown-linux-gnu")
    )]
    #[case::linux_aarch64_supergraph_v_2_0_7_fail(
        Plugin::Supergraph(FederationVersion::ExactFedTwo(Version::new(2, 0, 7))),
        "linux",
        "aarch64",
        None
    )]
    #[case::linux_aarch64_supergraph_latest_fed1(
        Plugin::Supergraph(FederationVersion::LatestFedOne),
        "linux",
        "aarch64",
        Some("aarch64-unknown-linux-gnu")
    )]
    #[case::linux_aarch64_supergraph_v_0_37_0(
        Plugin::Supergraph(FederationVersion::ExactFedOne(Version::new(0, 37, 0))),
        "linux",
        "aarch64",
        Some("aarch64-unknown-linux-gnu")
    )]
    #[case::linux_aarch64_supergraph_v_0_22_0_fail(
        Plugin::Supergraph(FederationVersion::ExactFedOne(Version::new(0, 22, 0))),
        "linux",
        "aarch64",
        None
    )]
    #[cfg(not(target_env = "musl"))]
    fn test_plugin_versions(
        #[case] plugin_version: Plugin,
        #[case] os: &str,
        #[case] arch: &str,
        #[case] expected_architecture: Option<&str>,
    ) {
        if let Some(expected_arch) = expected_architecture {
            assert_that!(plugin_version.get_arch_for_env(os, arch).unwrap())
                .is_equal_to(String::from(expected_arch));
        } else {
            assert_that!(plugin_version.get_arch_for_env(os, arch)).is_err();
        };
    }

    #[test]
    #[cfg(target_env = "musl")]
    fn test_plugin_version_should_fail() {
        Plugin::Router(RouterVersion::LatestTwo)
            .get_arch_for_env("", "")
            .unwrap_err();
    }
}
