use std::{env::consts, str::FromStr, sync::Arc};

use anyhow::{Context, anyhow};
use apollo_federation_types::config::{FederationVersion, PluginVersion, RouterVersion};
use binstall::{Installer, InstallerError, download::FileDownloadService};
use camino::Utf8PathBuf;
use http::StatusCode;
use rover_std::{Fs, sanitize_url, warnln};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{
    RoverError, RoverErrorSuggestion, RoverResult,
    federation::reject_federation_one,
    plugin::{
        error::{PluginFailure, RequestOrigin},
        version::{PluginName, VersionRequest},
    },
    utils::client::StudioClientConfig,
};

mod error;
mod mcp;
mod provenance;

pub(crate) use mcp::Version as McpServerVersion;
pub(crate) use provenance::{PluginLevel, PluginProvenance, PluginProvenanceTracker, PluginSource};

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
        self.name().to_string()
    }

    pub const fn name(&self) -> PluginName {
        match self {
            Self::Supergraph(_) => PluginName::Supergraph,
            Self::Router(_) => PluginName::Router,
            Self::McpServer(_) => PluginName::ApolloMcpServer,
        }
    }

    /// The version as it was asked for, in the shared plugin grammar.
    pub fn request(&self) -> VersionRequest {
        match self {
            Self::Supergraph(
                FederationVersion::ExactFedOne(v) | FederationVersion::ExactFedTwo(v),
            )
            | Self::Router(RouterVersion::Exact(v))
            | Self::McpServer(mcp::Version::Exact(v)) => VersionRequest::Exact(v.clone()),
            Self::Supergraph(FederationVersion::LatestFedOne) => VersionRequest::Major(0),
            Self::Supergraph(FederationVersion::LatestFedTwo)
            | Self::Router(RouterVersion::LatestTwo) => VersionRequest::Major(2),
            Self::Router(RouterVersion::LatestOne) => VersionRequest::Major(1),
            Self::McpServer(mcp::Version::Latest) => VersionRequest::Latest,
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
            "Your current architecture (os: {os}, arch: {arch}) does not support installation of this plugin."
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
                            //
                            // Every entry point that can produce a `Plugin::Supergraph` (CLI
                            // parsing via `Plugin::from_str`, composition, and `rover dev`'s
                            // live federation-version-change handling) rejects Federation 1
                            // before it gets here; kept because `FederationVersion` (from the
                            // external apollo-federation-types crate) still has Fed1 variants.
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
                            "Invalid version '{}' for 'supergraph' plugin. Must be 'latest-2' or an exact version preceded with an '='.",
                            plugin_version
                        )
                    })?;
                reject_federation_one(&federation_version)?;
                Ok(Plugin::Supergraph(federation_version))
            } else if plugin_name == "router" {
                let router_version = RouterVersion::from_str(&plugin_version).with_context({
                    || format!("Invalid version '{}' for 'router' plugin. Must be 'latest', '1', '2', or an exact version preceded with '=' (e.g. =2.0.0 for 2.x).", plugin_version)
                })?;
                Ok(Plugin::Router(router_version))
            } else if plugin_name == "apollo-mcp-server" {
                let mcp_version = mcp::Version::from_str(&plugin_version).with_context({
                    || format!("Invalid version '{}' for 'apollo-mcp-server' plugin. Must be 'latest' or an exact version preceded with 'v' or '=' (e.g. v1.0.0 or =1.0.0).", plugin_version)
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
    /// Where the version request came from, to name when an exact release
    /// turns out to have been withdrawn. `None` until a caller says.
    origin: Option<RequestOrigin>,
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
            origin: None,
        }
    }

    /// Records where the version request came from.
    pub fn requested_by(self, origin: RequestOrigin) -> Self {
        Self {
            origin: Some(origin),
            ..self
        }
    }

    pub async fn install(
        &self,
        plugin: &Plugin,
        skip_update: bool,
    ) -> RoverResult<PluginProvenance> {
        let (install_location, source) = match plugin {
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
                // Unreachable via `Plugin::from_str`, which rejects Federation 1 up front; kept
                // because `FederationVersion` still has Fed1 variants upstream.
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

        let name = plugin.get_name();
        let version = version_from_installed_path(&install_location, &name)?;
        // Always Global; no project-level install root exists yet.
        Ok(PluginProvenance::new(
            name,
            version,
            source,
            PluginLevel::Global,
            install_location,
        ))
    }

    async fn find_or_install_exact(
        &self,
        plugin: &Plugin,
        version: &str,
        skip_update: bool,
    ) -> RoverResult<(Utf8PathBuf, PluginSource)> {
        if skip_update {
            let exe = self
                .find_existing_exact(plugin, version)?
                .ok_or_else(|| skip_update_error(&plugin.get_name(), version))?;
            Ok((exe, PluginSource::Installed))
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
    ) -> RoverResult<(Utf8PathBuf, PluginSource)> {
        if skip_update {
            let exe = self
                .find_existing_latest_major(plugin, major_version)?
                .ok_or_else(|| skip_update_error(&plugin.get_name(), &major_version.to_string()))?;
            return Ok((exe, PluginSource::Installed));
        }
        match self.install_latest_major(plugin).await {
            Ok(Some((exe, source))) => Ok((exe, source)),
            Ok(None) => Err(could_not_install_plugin(
                &plugin.get_name(),
                &major_version.to_string(),
            )),
            Err(install_err) => match self.find_existing_latest_major(plugin, major_version) {
                Ok(Some(exe)) => {
                    tracing::debug!(
                        "could not download the '{}' plugin ({install_err}); falling back to {exe}",
                        plugin.get_name(),
                    );
                    warnln!(
                        "Couldn't download the latest '{}' plugin, so falling back to the already-installed '{}'. Re-run with a connection to the plugin registry to update.",
                        plugin.get_name(),
                        exe.file_name().unwrap_or_else(|| exe.as_str()),
                    );
                    Ok((exe, PluginSource::Fallback))
                }
                // Nothing usable on disk — surface the original download failure.
                _ => {
                    tracing::debug!(
                        "could not download the '{}' plugin ({install_err})",
                        plugin.get_name(),
                    );
                    Err(install_err)
                }
            },
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
                plugin_name,
                plugin_dir
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

    async fn install_latest_major(
        &self,
        plugin: &Plugin,
    ) -> RoverResult<Option<(Utf8PathBuf, PluginSource)>> {
        let (latest_version, resolved) = self.resolve_floating(plugin).await?;

        if let Ok(Some(exe)) = self.find_existing_exact(plugin, &latest_version)
            && !self.force
        {
            tracing::debug!("{} exists, skipping install", &exe);
            return Ok(Some((exe, PluginSource::Installed)));
        }
        // do the install.
        self.do_install(plugin, &resolved).await?;
        Ok(self
            .find_existing_exact(plugin, &latest_version)?
            .map(|exe| (exe, PluginSource::Downloaded)))
    }

    /// Asks the registry which release a floating request means, as it named
    /// it and parsed. A registry answer that isn't a version is a resolution
    /// failure here, not something the install trips over later.
    async fn resolve_floating(&self, plugin: &Plugin) -> RoverResult<(String, Version)> {
        let resolution_failed = |source: tower::BoxError| {
            RoverError::new(PluginFailure::Resolution {
                plugin: plugin.name(),
                requested: plugin.request(),
                source: source.into(),
            })
        };
        let named = self
            .installer
            .get_latest_plugin_version(
                self.client_config.plugin_version_service()?,
                &plugin.get_tarball_url()?,
            )
            .await
            .map_err(|err| resolution_failed(Box::new(err)))?;
        let parsed = parse_resolved_version(&named).map_err(|err| {
            resolution_failed(
                format!(
                    "the registry named `{named}` as the release, which isn't a version: {err}"
                )
                .into(),
            )
        })?;
        Ok((named, parsed))
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
    ) -> RoverResult<Option<(Utf8PathBuf, PluginSource)>> {
        if let Ok(Some(exe)) = self.find_existing_exact(plugin, version)
            && !self.force
        {
            tracing::debug!("{} exists, skipping install", &exe);
            return Ok(Some((exe, PluginSource::Installed)));
        }
        let Some(version) = plugin.request().exact().cloned() else {
            return Err(could_not_install_plugin(&plugin.get_name(), version));
        };
        Ok(self
            .do_install(plugin, &version)
            .await?
            .map(|exe| (exe, PluginSource::Downloaded)))
    }

    /// Sorts an installer error into the download or installation failure it is.
    fn install_failure(
        &self,
        plugin: &Plugin,
        version: &Version,
        err: InstallerError,
    ) -> RoverError {
        match err {
            // The download error itself, not the installer's wrapper around it,
            // which would only repeat that the download failed.
            InstallerError::FileDownloadError(source) => RoverError::new(PluginFailure::Download {
                plugin: plugin.name(),
                requested: plugin.request(),
                version: version.clone(),
                source: Arc::new(*source),
            }),
            // Without a root to name, there's no installation failure to report.
            err => match self.installer.bin_dir_location() {
                Ok(install_root) => RoverError::new(PluginFailure::Installation {
                    plugin: plugin.name(),
                    requested: plugin.request(),
                    version: version.clone(),
                    install_root,
                    source: Arc::new(err),
                }),
                Err(_) => RoverError::new(err),
            },
        }
    }

    async fn do_install(
        &self,
        plugin: &Plugin,
        version: &Version,
    ) -> RoverResult<Option<Utf8PathBuf>> {
        let plugin_name = plugin.get_name();
        let plugin_tarball_url = plugin.get_tarball_url()?;
        // only print the download message if the username and password have been stripped from the URL
        if let Some(sanitized_url) = sanitize_url(&plugin_tarball_url) {
            eprintln!("downloading the '{plugin_name}' plugin from {sanitized_url}");
        } else {
            eprintln!("downloading the '{plugin_name}' plugin");
        }
        let file_download_service = FileDownloadService::builder()
            .http_service(self.client_config.download_service()?)
            // Match the per-attempt timeout to the configured download timeout so it
            // isn't capped by FileDownloadService's shorter default.
            .timeout_duration(*self.client_config.download_timeout())
            .build();
        let result = self
            .installer
            .install_plugin(
                &plugin_name,
                &plugin_tarball_url,
                file_download_service,
                &format!("v{version}"),
            )
            .await;
        match result {
            Ok(installed) => Ok(installed),
            Err(err) if plugin.request().exact().is_some() && is_not_served(&err) => {
                Err(self.not_served(plugin, version, err).await)
            }
            Err(err) => Err(self.install_failure(plugin, version, err)),
        }
    }

    /// An exact release the registry has no artifact for either never existed
    /// or was withdrawn, and only the registry knows which: it answers
    /// `410 Gone` for a withdrawn release and `404 Not Found` for one it never
    /// published. A 404 isn't second-guessed, since a release missing for
    /// this platform, or skipped in a sequence, looks the same from here.
    async fn not_served(
        &self,
        plugin: &Plugin,
        version: &Version,
        err: InstallerError,
    ) -> RoverError {
        let withdrawn = err.download_status() == Some(StatusCode::GONE);
        match &self.origin {
            Some(origin) if withdrawn => RoverError::new(PluginFailure::NoLongerServed {
                plugin: plugin.name(),
                version: version.clone(),
                origin: origin.clone(),
                newest_in_major: self.newest_in_major(plugin, version.major).await,
            }),
            _ => RoverError::new(PluginFailure::Resolution {
                plugin: plugin.name(),
                requested: plugin.request(),
                source: Arc::new(err),
            }),
        }
    }

    /// The newest release in `major`, or `None` when the registry can't say.
    async fn newest_in_major(&self, plugin: &Plugin, major: u64) -> Option<Version> {
        let floating = match (plugin, major) {
            (Plugin::Supergraph(_), 2) => Plugin::Supergraph(FederationVersion::LatestFedTwo),
            (Plugin::Router(_), 1) => Plugin::Router(RouterVersion::LatestOne),
            (Plugin::Router(_), 2) => Plugin::Router(RouterVersion::LatestTwo),
            // The registry's only `apollo-mcp-server` alias spans majors.
            (Plugin::McpServer(_), _) => Plugin::McpServer(mcp::Version::Latest),
            _ => return None,
        };
        let (_, newest) = self.resolve_floating(&floating).await.ok()?;
        Some(newest).filter(|newest| newest.major == major)
    }
}

/// Whether the registry refused the artifact as missing or withdrawn.
fn is_not_served(err: &InstallerError) -> bool {
    matches!(
        err.download_status(),
        Some(StatusCode::NOT_FOUND | StatusCode::GONE)
    )
}

/// A registry-reported or URL-derived version, which may carry a `v` prefix.
fn parse_resolved_version(version: &str) -> Result<Version, semver::Error> {
    Version::parse(version.strip_prefix('v').unwrap_or(version))
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
            let file_name = installed_plugin.file_name();
            let file_name = file_name
                .strip_suffix(std::env::consts::EXE_SUFFIX)
                .unwrap_or(file_name);
            let splits: Vec<String> = file_name.split("-v").map(|x| x.to_string()).collect();
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
        .map(|v| {
            plugin_dir.join(format!(
                "{}-v{}{}",
                plugin_name,
                v,
                std::env::consts::EXE_SUFFIX
            ))
        })
        .collect();
    Ok(installed_plugins)
}

fn version_from_installed_path(path: &Utf8PathBuf, plugin_name: &str) -> RoverResult<Version> {
    let file_name = path.file_name().ok_or_else(|| {
        RoverError::new(anyhow!(
            "Could not determine a plugin name from the path '{path}'."
        ))
    })?;
    let without_exe = file_name
        .strip_suffix(std::env::consts::EXE_SUFFIX)
        .unwrap_or(file_name);
    let without_prefix = without_exe
        .strip_prefix(&format!("{plugin_name}-v"))
        .unwrap_or(without_exe);
    Version::parse(without_prefix).map_err(|_| {
        RoverError::new(anyhow!(
            "Could not parse a version from the installed plugin path '{path}'."
        ))
    })
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
        let mut err = RoverError::new(anyhow!("Could not find plugin at {}", maybe_plugin));
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
    use speculoos::prelude::*;

    use super::*;

    mod plugin_from_str {
        use super::*;

        #[rstest::rstest]
        // Valid supergraph (FederationVersion from apollo-federation-types accepts latest-2, =X.Y.Z;
        // Federation 1 versions parse but are rejected below in `federation_one_is_rejected`)
        #[case::supergraph_latest_2("supergraph@latest-2")]
        #[case::supergraph_exact_fed2("supergraph@=2.8.0")]
        // Valid router (RouterVersion accepts "1", "2", "latest", or =X.Y.Z for exact; 1.x and 2.x)
        #[case::router_latest("router@latest")]
        #[case::router_1("router@1")]
        #[case::router_2("router@2")]
        #[case::router_equals_1("router@=1.0.0")]
        // Valid apollo-mcp-server
        #[case::mcp_latest("apollo-mcp-server@latest")]
        #[case::mcp_v("apollo-mcp-server@v1.0.0")]
        #[case::mcp_equals("apollo-mcp-server@=1.0.0")]
        fn valid_plugin_parses(#[case] input: &str) {
            let plugin = Plugin::from_str(input).expect("should parse");
            match input {
                s if s.starts_with("supergraph@") => {
                    assert!(matches!(plugin, Plugin::Supergraph(_)))
                }
                s if s.starts_with("router@") => {
                    assert!(matches!(plugin, Plugin::Router(_)));
                    if s.contains("2") && !s.contains("1.0") {
                        match &plugin {
                            Plugin::Router(RouterVersion::LatestTwo) => {}
                            Plugin::Router(RouterVersion::Exact(v)) => assert_eq!(v.major, 2),
                            _ => panic!("expected router 2.x variant"),
                        }
                    }
                }
                s if s.starts_with("apollo-mcp-server@") => {
                    assert!(matches!(plugin, Plugin::McpServer(_)))
                }
                _ => {}
            }
        }

        #[test]
        fn router_2x_parses_as_expected() {
            // apollo-federation-types accepts "2" for latest router 2.x (RouterVersion::LatestTwo)
            let p2 = Plugin::from_str("router@2").expect("should parse");
            assert!(matches!(p2, Plugin::Router(RouterVersion::LatestTwo)));
        }

        #[test]
        fn case_insensitivity_plugin_name() {
            let p = Plugin::from_str("Supergraph@latest-2").expect("should parse");
            assert!(matches!(
                p,
                Plugin::Supergraph(FederationVersion::LatestFedTwo)
            ));
        }

        #[rstest::rstest]
        #[case::missing_at("supergraph")]
        #[case::missing_at_version("supergraph2.8.0")]
        fn invalid_malformed_plugin(#[case] input: &str) {
            let err = Plugin::from_str(input).unwrap_err();
            assert_that!(err.to_string()).contains("name");
            assert_that!(err.to_string()).contains("version");
        }

        #[test]
        fn invalid_plugin_name() {
            let err = Plugin::from_str("badname@1.0.0").unwrap_err();
            assert_that!(err.to_string()).contains("Invalid plugin name");
            assert_that!(err.to_string()).contains("apollo-mcp-server");
            assert_that!(err.to_string()).contains("supergraph");
            assert_that!(err.to_string()).contains("router");
        }

        #[rstest::rstest]
        #[case::supergraph_no_equals("supergraph@2.8.0")]
        #[case::supergraph_latest_plain("supergraph@latest")]
        #[case::router_no_prefix("router@1.0.0")]
        #[case::mcp_no_prefix("apollo-mcp-server@1.0.0")]
        fn invalid_version_per_plugin(#[case] input: &str) {
            let err = Plugin::from_str(input).unwrap_err();
            assert_that!(err.to_string()).is_not_equal_to("Invalid plugin name".to_string());
        }

        #[rstest::rstest]
        #[case::latest_0("supergraph@latest-0")]
        #[case::exact("supergraph@=0.36.0")]
        fn federation_one_is_rejected(#[case] input: &str) {
            use crate::federation::FederationOneUnsupported;

            let err = Plugin::from_str(input).unwrap_err();
            assert_that!(err.to_string()).is_equal_to(FederationOneUnsupported.to_string());
        }
    }

    #[rstest::rstest]
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
        use speculoos::{assert_that, prelude::ResultAssertions};

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

    #[cfg(not(target_env = "musl"))]
    mod a_release_the_registry_does_not_serve {
        use std::time::Duration;

        use houston::Config;
        use httpmock::{Method, MockServer};
        use rstest::rstest;

        use super::*;
        use crate::{
            RoverErrorCode,
            utils::client::{ClientBuilder, ClientTimeout},
        };

        /// What the registry answers for the exact artifact, and what it says
        /// is the newest release in the major (`None`: it can't say).
        async fn install_exact(
            version: &str,
            artifact_status: u16,
            newest_in_major: Option<&str>,
            origin: Option<RequestOrigin>,
        ) -> RoverError {
            let server = MockServer::start();
            let host = format!("http://{}", server.address());
            server.mock(|when, then| {
                when.method(Method::GET)
                    .path_includes(format!("/v{version}"));
                then.status(artifact_status);
            });
            server.mock(|when, then| {
                when.method(Method::HEAD).path_includes("/latest-2");
                match newest_in_major {
                    Some(newest) => then.status(302).header("X-Version", format!("v{newest}")),
                    None => then.status(404),
                };
            });

            let home_dir = tempfile::tempdir().unwrap();
            let home = Utf8PathBuf::from_path_buf(home_dir.path().to_path_buf()).unwrap();
            let client_config = StudioClientConfig::new(
                None,
                Config {
                    home: home.join("config"),
                    override_api_key: None,
                    override_client_credentials_token: None,
                },
                false,
                ClientBuilder::default(),
                ClientTimeout::new(1),
            );
            let installer = Installer {
                binary_name: "rover".to_string(),
                force_install: false,
                executable_location: home.join("rover"),
                override_install_path: Some(home),
            };
            let plugin = Plugin::Supergraph(FederationVersion::ExactFedTwo(
                Version::parse(version).unwrap(),
            ));

            temp_env::async_with_vars(
                [
                    ("APOLLO_ROVER_DOWNLOAD_HOST", Some(host)),
                    ("APOLLO_NODE_MODULES_BIN_DIR", None),
                ],
                async {
                    let plugin_installer = PluginInstaller::new(client_config, installer, false);
                    let plugin_installer = match origin {
                        Some(origin) => plugin_installer.requested_by(origin),
                        None => plugin_installer,
                    };
                    plugin_installer
                        .install(&plugin, false)
                        .await
                        .expect_err("the registry serves no such artifact")
                },
            )
            .await
        }

        fn reported(error: &RoverError) -> (Option<RoverErrorCode>, String, Vec<String>) {
            (
                error.code(),
                error.message(),
                error
                    .suggestions()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            )
        }

        #[rstest]
        #[case::gone_with_a_newer_release(
            410,
            Some("2.9.5"),
            "The `supergraph` plugin v2.9.3, requested by `rover install --plugin`, is no longer available from the plugin registry. The newest available 2.x is v2.9.5.",
            "Run `rover install --plugin supergraph@=2.9.5`."
        )]
        #[case::gone_with_no_listing(
            410,
            None,
            "The `supergraph` plugin v2.9.3, requested by `rover install --plugin`, is no longer available from the plugin registry.",
            "Run `rover install --plugin supergraph@2` to use the newest available 2.x."
        )]
        #[tokio::test]
        #[timeout(Duration::from_secs(15))]
        async fn a_withdrawn_release_is_reported_as_no_longer_served(
            #[case] artifact_status: u16,
            #[case] newest_in_major: Option<&str>,
            #[case] message: &str,
            #[case] next_step: &str,
        ) {
            let error = install_exact(
                "2.9.3",
                artifact_status,
                newest_in_major,
                Some(RequestOrigin::PluginArgument),
            )
            .await;

            assert_that!(reported(&error)).is_equal_to((
                Some(RoverErrorCode::E051),
                message.to_string(),
                vec![next_step.to_string()],
            ));
        }

        /// A 404 is the registry saying it never published the release, which
        /// is class 1's "no release matches", not a withdrawal — wherever the
        /// version falls relative to what it has published. A withdrawal with
        /// no recorded origin can't be reported as one yet, so it's class 1
        /// too.
        #[rstest]
        #[case::above_the_newest_release(
            "2.9.9",
            404,
            Some("2.9.5"),
            Some(RequestOrigin::PluginArgument)
        )]
        #[case::below_the_newest_release(
            "2.9.9",
            404,
            Some("2.10.0"),
            Some(RequestOrigin::PluginArgument)
        )]
        #[case::with_no_listing("2.9.9", 404, None, Some(RequestOrigin::PluginArgument))]
        #[case::withdrawn_with_no_origin("2.9.9", 410, Some("2.10.0"), None)]
        #[tokio::test]
        #[timeout(Duration::from_secs(15))]
        async fn a_release_that_never_existed_is_a_resolution_failure(
            #[case] version: &str,
            #[case] artifact_status: u16,
            #[case] newest_in_major: Option<&str>,
            #[case] origin: Option<RequestOrigin>,
        ) {
            let error = install_exact(version, artifact_status, newest_in_major, origin).await;

            assert_that!(reported(&error)).is_equal_to((
                Some(RoverErrorCode::E048),
                "Couldn't resolve a release of the `supergraph` plugin matching `=2.9.9` from the plugin registry.".to_string(),
                vec!["Make sure the plugin registry is reachable and that `supergraph` has a release matching `=2.9.9`, then re-run the command. If you use a registry other than Apollo's, check that `APOLLO_ROVER_DOWNLOAD_HOST` points at it.".to_string()],
            ));
        }
    }
}
