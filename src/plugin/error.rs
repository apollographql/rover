//! The error contract for plugin failures.
//!
//! Each class of plugin failure has its own variant of [`PluginFailure`]:
//! resolution, download, installation, and a withdrawn release so far, with
//! the never-download, checksum, and manifest classes to follow. Every variant
//! has its own stable [`RoverErrorCode`] and a [`PluginNextStep`] naming the
//! plugin and what to do about it. A [`PluginFailure`] anywhere in an error's
//! cause chain decides that error's code and suggestion, so a caller that
//! wraps one keeps both.
//!
//! Wrap a [`PluginFailure`] with `anyhow` context or as a `thiserror`
//! `#[source]`, never `#[error(transparent)]`: a transparent wrapper hands
//! back the failure's own source, which hides the failure from the chain.
//!
//! Adding a failure class means adding a variant here, the arms the compiler
//! then asks for in this file's matches, a new [`RoverErrorCode`], and that
//! code's explanation. Nothing outside this file and the code list changes.

use std::{error::Error, fmt, sync::Arc};

use camino::Utf8PathBuf;
use semver::Version;
use serde::Serialize;

use super::version::{PluginName, VersionRequest};
use crate::{RoverErrorCode, utils::client::DOWNLOAD_REQUEST_TIMEOUT};

/// What caused a [`PluginFailure`]. Shared rather than owned so that the
/// failure can be cloned into the error types that carry it.
pub type PluginFailureCause = Arc<dyn Error + Send + Sync + 'static>;

/// A plugin could not be obtained.
#[derive(Debug, Clone)]
pub enum PluginFailure {
    /// The registry could not say which release a request means: it was
    /// unreachable, or no release matches the request.
    Resolution {
        plugin: PluginName,
        requested: VersionRequest,
        source: PluginFailureCause,
    },

    /// A release was chosen, but its artifact could not be downloaded.
    Download {
        plugin: PluginName,
        requested: VersionRequest,
        version: Version,
        source: PluginFailureCause,
    },

    /// The artifact was downloaded, but could not be extracted or written
    /// into the install root.
    Installation {
        plugin: PluginName,
        requested: VersionRequest,
        version: Version,
        install_root: Utf8PathBuf,
        source: PluginFailureCause,
    },

    /// An exact release the registry once served, and no longer does.
    NoLongerServed {
        plugin: PluginName,
        version: Version,
        origin: RequestOrigin,
        /// The newest release in `version`'s major, when the registry could
        /// say. Ignored unless it is in that major and is not `version`.
        newest_in_major: Option<Version>,
    },
}

impl fmt::Display for PluginFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resolution {
                plugin, requested, ..
            } => write!(
                f,
                "Couldn't resolve a release of the `{plugin}` plugin matching `{requested}` from the plugin registry."
            ),
            Self::Download {
                plugin, version, ..
            } => write!(f, "Couldn't download the `{plugin}` plugin v{version}."),
            Self::Installation {
                plugin,
                version,
                install_root,
                ..
            } => write!(
                f,
                "Couldn't install the `{plugin}` plugin v{version} into `{install_root}`."
            ),
            Self::NoLongerServed {
                plugin,
                version,
                origin,
                newest_in_major,
            } => {
                write!(
                    f,
                    "The `{plugin}` plugin v{version}, {origin}, is no longer available from the plugin registry."
                )?;
                if let Some(newest) = replacement(version, newest_in_major.as_ref()) {
                    write!(f, " The newest available {}.x is v{newest}.", version.major)?;
                }
                Ok(())
            }
        }
    }
}

/// Written by hand rather than derived so that the cause chain yields the
/// cause itself, not the [`Arc`] around it, and a caller can still downcast it
/// to its concrete type.
impl Error for PluginFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resolution { source, .. }
            | Self::Download { source, .. }
            | Self::Installation { source, .. } => Some(&**source),
            Self::NoLongerServed { .. } => None,
        }
    }
}

impl PluginFailure {
    /// The plugin that could not be obtained.
    ///
    /// `None` for a failure that names no single plugin, such as a malformed
    /// manifest; every class so far names one.
    pub const fn plugin(&self) -> Option<PluginName> {
        match self {
            Self::Resolution { plugin, .. }
            | Self::Download { plugin, .. }
            | Self::Installation { plugin, .. }
            | Self::NoLongerServed { plugin, .. } => Some(*plugin),
        }
    }

    /// The version as it was asked for, before any resolution.
    ///
    /// `None` for a failure that names no request, such as a malformed
    /// manifest; every class so far names one.
    pub fn requested(&self) -> Option<VersionRequest> {
        match self {
            Self::Resolution { requested, .. }
            | Self::Download { requested, .. }
            | Self::Installation { requested, .. } => Some(requested.clone()),
            Self::NoLongerServed { version, .. } => Some(VersionRequest::Exact(version.clone())),
        }
    }

    /// The stable code this failure is reported under.
    pub const fn code(&self) -> RoverErrorCode {
        match self {
            Self::Resolution { .. } => RoverErrorCode::E048,
            Self::Download { .. } => RoverErrorCode::E049,
            Self::Installation { .. } => RoverErrorCode::E050,
            Self::NoLongerServed { .. } => RoverErrorCode::E051,
        }
    }

    /// What the user should do next.
    pub fn next_step(&self) -> PluginNextStep {
        match self {
            Self::Resolution {
                plugin, requested, ..
            } => PluginNextStep::CheckRegistry {
                plugin: *plugin,
                requested: requested.clone(),
            },
            Self::Download {
                plugin, version, ..
            } => PluginNextStep::RetryDownload {
                plugin: *plugin,
                version: version.clone(),
            },
            Self::Installation {
                plugin,
                version,
                install_root,
                ..
            } => PluginNextStep::FixInstallRoot {
                plugin: *plugin,
                version: version.clone(),
                install_root: install_root.clone(),
            },
            Self::NoLongerServed {
                plugin,
                version,
                origin,
                newest_in_major,
            } => PluginNextStep::RequestAnotherVersion {
                plugin: *plugin,
                origin: origin.clone(),
                request: replacement(version, newest_in_major.as_ref()).map_or_else(
                    || floating_request(*plugin, version.major),
                    |replacement| Some(VersionRequest::Exact(replacement.clone())),
                ),
            },
        }
    }
}

/// The release to suggest in place of withdrawn `version`: the registry's
/// newest in the same major, unless that is `version` itself.
fn replacement<'a>(version: &Version, newest: Option<&'a Version>) -> Option<&'a Version> {
    newest.filter(|newest| newest.major == version.major && *newest != version)
}

/// The newest release in `major`, spelled so today's parsers accept it, or
/// `None` when no floating form they accept means that. The
/// `apollo-mcp-server` parser takes no bare major, only `latest`, which can
/// cross into a newer major; the suggestion says so.
const fn floating_request(plugin: PluginName, major: u64) -> Option<VersionRequest> {
    match (plugin, major) {
        (PluginName::ApolloMcpServer, _) => Some(VersionRequest::Latest),
        (PluginName::Supergraph, 2) | (PluginName::Router, 1 | 2) => {
            Some(VersionRequest::Major(major))
        }
        _ => None,
    }
}

/// Where a version request came from, phrased to follow the plugin and
/// version it produced: "the `supergraph` plugin v2.9.3, set by ...".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum RequestOrigin {
    /// `rover install --plugin <name>@<version>`.
    PluginArgument,
    /// A version flag such as `--federation-version`.
    Flag(&'static str),
    /// An environment variable such as `APOLLO_ROVER_DEV_ROUTER_VERSION`.
    EnvVar(&'static str),
    /// `federation_version` in `supergraph.yaml`.
    SupergraphConfig,
}

impl fmt::Display for RequestOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PluginArgument => f.write_str("requested by `rover install --plugin`"),
            Self::Flag(flag) => write!(f, "requested by `{flag}`"),
            Self::EnvVar(var) => write!(f, "set by `{var}`"),
            Self::SupergraphConfig => {
                f.write_str("set by `federation_version` in `supergraph.yaml`")
            }
        }
    }
}

/// The concrete next step a [`PluginFailure`] suggests. Each names the plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum PluginNextStep {
    CheckRegistry {
        plugin: PluginName,
        requested: VersionRequest,
    },
    RetryDownload {
        plugin: PluginName,
        version: Version,
    },
    FixInstallRoot {
        plugin: PluginName,
        version: Version,
        install_root: Utf8PathBuf,
    },
    RequestAnotherVersion {
        plugin: PluginName,
        origin: RequestOrigin,
        /// What to ask for instead: an exact release when the registry named
        /// one, otherwise the newest release in the withdrawn one's major, or
        /// `None` when there's no form today's parsers accept for that.
        request: Option<VersionRequest>,
    },
}

impl fmt::Display for PluginNextStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckRegistry { plugin, requested } => write!(
                f,
                "Make sure the plugin registry is reachable and that `{plugin}` has a release matching `{requested}`, then re-run the command. If you use a registry other than Apollo's, check that `APOLLO_ROVER_DOWNLOAD_HOST` points at it."
            ),
            Self::RetryDownload { plugin, version } => write!(
                f,
                "Re-run the command to retry downloading `{plugin}` v{version}. If the download keeps failing or timing out, check your connection to the plugin registry, or allow it longer by passing `--client-timeout` a value above the {}-second default for plugin downloads.",
                DOWNLOAD_REQUEST_TIMEOUT.as_secs()
            ),
            Self::FixInstallRoot {
                plugin,
                version,
                install_root,
            } => write!(
                f,
                "Make sure `{install_root}` is writable and has free space, then run `rover install --plugin {plugin}@={version} --force --elv2-license accept` to reinstall it."
            ),
            Self::RequestAnotherVersion {
                plugin,
                origin,
                request,
            } => {
                let Some(request) = request else {
                    return write!(
                        f,
                        "Request an exact `{plugin}` release that the plugin registry still serves."
                    );
                };
                let newest = match request {
                    VersionRequest::Exact(_) => String::new(),
                    VersionRequest::Major(major) => {
                        format!(" to use the newest available {major}.x")
                    }
                    VersionRequest::Latest => {
                        " to use the newest available release, which may be a newer major version"
                            .to_string()
                    }
                };
                match origin {
                    RequestOrigin::PluginArgument => write!(
                        f,
                        "Run `rover install --plugin {plugin}@{request}`{newest}."
                    ),
                    RequestOrigin::Flag(flag) => {
                        write!(f, "Re-run with `{flag} {request}`{newest}.")
                    }
                    // A bare exact version is the one form every version
                    // variable accepts; unsetting one falls back to the
                    // next source of the version.
                    RequestOrigin::EnvVar(var) => match request {
                        VersionRequest::Exact(version) => {
                            write!(f, "Set `{var}` to `{version}`.")
                        }
                        _ => write!(
                            f,
                            "Unset `{var}`, or set it to an exact version the plugin registry still serves."
                        ),
                    },
                    RequestOrigin::SupergraphConfig => write!(
                        f,
                        "Set `federation_version: {request}` in `supergraph.yaml`{newest}."
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;
    use crate::RoverError;

    fn cause(message: &str) -> PluginFailureCause {
        Arc::new(io::Error::other(message.to_string()))
    }

    fn v(version: &str) -> Version {
        Version::parse(version).unwrap()
    }

    fn resolution() -> PluginFailure {
        PluginFailure::Resolution {
            plugin: PluginName::Supergraph,
            requested: VersionRequest::Exact(v("2.9.9")),
            source: cause("Bad Status code: 404 Not Found"),
        }
    }

    fn download() -> PluginFailure {
        PluginFailure::Download {
            plugin: PluginName::Router,
            requested: VersionRequest::Major(2),
            version: v("2.1.0"),
            source: cause("Request timed out"),
        }
    }

    fn installation() -> PluginFailure {
        PluginFailure::Installation {
            plugin: PluginName::ApolloMcpServer,
            requested: VersionRequest::Latest,
            version: v("1.0.0"),
            install_root: Utf8PathBuf::from("/home/me/.rover/bin"),
            source: cause("Permission denied (os error 13)"),
        }
    }

    fn no_longer_served(origin: RequestOrigin, newest: Option<&str>) -> PluginFailure {
        PluginFailure::NoLongerServed {
            plugin: PluginName::Supergraph,
            version: v("2.9.3"),
            origin,
            newest_in_major: newest.map(v),
        }
    }

    /// The error exactly as `rover` prints it, uncoloured.
    fn printed(failure: PluginFailure) -> String {
        console::strip_ansi_codes(&RoverError::new(failure).to_string()).into_owned()
    }

    #[rstest]
    #[case::resolution(
        resolution(),
        "error[E048]: Couldn't resolve a release of the `supergraph` plugin matching `=2.9.9` from the plugin registry.\n\
         \n\
         Caused by:\n    \
         Bad Status code: 404 Not Found\n        \
         Make sure the plugin registry is reachable and that `supergraph` has a release matching `=2.9.9`, then re-run the command. If you use a registry other than Apollo's, check that `APOLLO_ROVER_DOWNLOAD_HOST` points at it.\n"
    )]
    #[case::download(
        download(),
        "error[E049]: Couldn't download the `router` plugin v2.1.0.\n\
         \n\
         Caused by:\n    \
         Request timed out\n        \
         Re-run the command to retry downloading `router` v2.1.0. If the download keeps failing or timing out, check your connection to the plugin registry, or allow it longer by passing `--client-timeout` a value above the 300-second default for plugin downloads.\n"
    )]
    #[case::installation(
        installation(),
        "error[E050]: Couldn't install the `apollo-mcp-server` plugin v1.0.0 into `/home/me/.rover/bin`.\n\
         \n\
         Caused by:\n    \
         Permission denied (os error 13)\n        \
         Make sure `/home/me/.rover/bin` is writable and has free space, then run `rover install --plugin apollo-mcp-server@=1.0.0 --force --elv2-license accept` to reinstall it.\n"
    )]
    #[case::no_longer_served(
        no_longer_served(RequestOrigin::PluginArgument, Some("2.9.5")),
        "error[E051]: The `supergraph` plugin v2.9.3, requested by `rover install --plugin`, is no longer available from the plugin registry. The newest available 2.x is v2.9.5.\n        \
         Run `rover install --plugin supergraph@=2.9.5`.\n"
    )]
    fn each_failure_prints_its_code_message_cause_and_next_step(
        #[case] failure: PluginFailure,
        #[case] expected: &str,
    ) {
        assert_that!(printed(failure)).is_equal_to(expected.to_string());
    }

    #[rstest]
    #[case::resolution(
        resolution(),
        serde_json::json!({
            "message": "Couldn't resolve a release of the `supergraph` plugin matching `=2.9.9` from the plugin registry.",
            "causes": ["Bad Status code: 404 Not Found"],
            "code": "E048",
        })
    )]
    #[case::download(
        download(),
        serde_json::json!({
            "message": "Couldn't download the `router` plugin v2.1.0.",
            "causes": ["Request timed out"],
            "code": "E049",
        })
    )]
    #[case::installation(
        installation(),
        serde_json::json!({
            "message": "Couldn't install the `apollo-mcp-server` plugin v1.0.0 into `/home/me/.rover/bin`.",
            "causes": ["Permission denied (os error 13)"],
            "code": "E050",
        })
    )]
    #[case::no_longer_served(
        no_longer_served(RequestOrigin::PluginArgument, None),
        serde_json::json!({
            "message": "The `supergraph` plugin v2.9.3, requested by `rover install --plugin`, is no longer available from the plugin registry.",
            "code": "E051",
        })
    )]
    fn each_failure_is_reported_under_its_own_code_in_json(
        #[case] failure: PluginFailure,
        #[case] expected: serde_json::Value,
    ) {
        let value = serde_json::to_value(RoverError::new(failure)).unwrap();

        assert_that!(value).is_equal_to(expected);
    }

    /// `rover explain` and the generated error reference both read the
    /// code's explanation, so every failure's code must have one.
    #[rstest]
    #[case::resolution(resolution(), include_str!("../error/metadata/codes/E048.md"))]
    #[case::download(download(), include_str!("../error/metadata/codes/E049.md"))]
    #[case::installation(installation(), include_str!("../error/metadata/codes/E050.md"))]
    #[case::no_longer_served(
        no_longer_served(RequestOrigin::PluginArgument, None),
        include_str!("../error/metadata/codes/E051.md")
    )]
    fn every_code_is_explained(#[case] failure: PluginFailure, #[case] explanation: &str) {
        let code = failure.code();

        assert_that!(code.explain()).is_equal_to(format!("**{code}**\n\n{explanation}\n\n"));
    }

    /// A caller that wraps the failure in its own error must not lose the
    /// code or the next step, since that is how on-the-fly commands report it.
    ///
    /// Both ways a caller can wrap it are covered: `anyhow` context, and a
    /// `thiserror` variant holding it as `#[source]`. A variant marked
    /// `#[error(transparent)]` hides it from the cause chain and loses both.
    #[rstest]
    #[case::context(anyhow::Error::new(download()).context("unable to find dependency"))]
    #[case::source(anyhow::Error::new(Wrapper(download())))]
    fn a_wrapped_failure_keeps_its_code_and_next_step(#[case] wrapped: anyhow::Error) {
        let error = RoverError::new(wrapped);

        assert_that!(error.code()).is_equal_to(Some(RoverErrorCode::E049));
        assert_that!(
            error
                .suggestions()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        )
        .is_equal_to(vec![download().next_step().to_string()]);
    }

    #[test]
    fn the_cause_downcasts_to_its_concrete_type() {
        let failure = download();
        let cause = failure
            .source()
            .and_then(|cause| cause.downcast_ref::<io::Error>());

        assert_that!(cause.map(ToString::to_string))
            .is_equal_to(Some("Request timed out".to_string()));
    }

    #[derive(Debug, thiserror::Error)]
    #[error("unable to find dependency")]
    struct Wrapper(#[source] PluginFailure);

    #[rstest]
    #[case::argument_with_listing(
        RequestOrigin::PluginArgument,
        Some("2.9.5"),
        "Run `rover install --plugin supergraph@=2.9.5`."
    )]
    #[case::argument_without_listing(
        RequestOrigin::PluginArgument,
        None,
        "Run `rover install --plugin supergraph@2` to use the newest available 2.x."
    )]
    #[case::flag(
        RequestOrigin::Flag("--federation-version"),
        Some("2.9.5"),
        "Re-run with `--federation-version =2.9.5`."
    )]
    #[case::the_newest_is_the_withdrawn_release(
        RequestOrigin::PluginArgument,
        Some("2.9.3"),
        "Run `rover install --plugin supergraph@2` to use the newest available 2.x."
    )]
    #[case::the_newest_is_in_another_major(
        RequestOrigin::PluginArgument,
        Some("3.0.0"),
        "Run `rover install --plugin supergraph@2` to use the newest available 2.x."
    )]
    #[case::env_var(
        RequestOrigin::EnvVar("APOLLO_ROVER_DEV_ROUTER_VERSION"),
        None,
        "Unset `APOLLO_ROVER_DEV_ROUTER_VERSION`, or set it to an exact version the plugin registry still serves."
    )]
    #[case::env_var_exact(
        RequestOrigin::EnvVar("APOLLO_ROVER_DEV_ROUTER_VERSION"),
        Some("2.9.5"),
        "Set `APOLLO_ROVER_DEV_ROUTER_VERSION` to `2.9.5`."
    )]
    #[case::supergraph_config(
        RequestOrigin::SupergraphConfig,
        Some("2.9.5"),
        "Set `federation_version: =2.9.5` in `supergraph.yaml`."
    )]
    fn a_withdrawn_version_suggests_changing_the_request_where_it_was_made(
        #[case] origin: RequestOrigin,
        #[case] newest: Option<&str>,
        #[case] expected: &str,
    ) {
        let step = no_longer_served(origin, newest).next_step();

        assert_that!(step.to_string()).is_equal_to(expected.to_string());
    }

    /// The registry's newest can be the withdrawn release itself, or sit in
    /// another major; neither is a replacement worth naming.
    #[rstest]
    #[case::itself("2.9.3")]
    #[case::another_major("3.0.0")]
    fn a_withdrawn_version_names_no_unusable_replacement(#[case] newest: &str) {
        assert_that!(no_longer_served(RequestOrigin::PluginArgument, Some(newest)).to_string())
            .is_equal_to(
                "The `supergraph` plugin v2.9.3, requested by `rover install --plugin`, is no longer available from the plugin registry."
                    .to_string(),
            );
    }

    /// No floating form today's parsers accept means "the newest 3.x", so the
    /// suggestion asks for an exact release rather than one they'd reject.
    #[test]
    fn a_withdrawn_release_with_no_accepted_floating_form_asks_for_an_exact_one() {
        let step = PluginFailure::NoLongerServed {
            plugin: PluginName::Router,
            version: v("3.0.1"),
            origin: RequestOrigin::PluginArgument,
            newest_in_major: None,
        }
        .next_step();

        assert_that!(step.to_string()).is_equal_to(
            "Request an exact `router` release that the plugin registry still serves.".to_string(),
        );
    }

    /// The `apollo-mcp-server` parser has no bare-major form, so its fallback
    /// has to be one it accepts.
    #[test]
    fn a_withdrawn_mcp_server_falls_back_to_latest() {
        let step = PluginFailure::NoLongerServed {
            plugin: PluginName::ApolloMcpServer,
            version: v("1.0.3"),
            origin: RequestOrigin::PluginArgument,
            newest_in_major: None,
        }
        .next_step();

        assert_that!(step.to_string()).is_equal_to(
            "Run `rover install --plugin apollo-mcp-server@latest` to use the newest available release, which may be a newer major version."
                .to_string(),
        );
    }

    #[rstest]
    #[case::argument(
        RequestOrigin::PluginArgument,
        "The `supergraph` plugin v2.9.3, requested by `rover install --plugin`, is no longer available from the plugin registry."
    )]
    #[case::flag(
        RequestOrigin::Flag("--federation-version"),
        "The `supergraph` plugin v2.9.3, requested by `--federation-version`, is no longer available from the plugin registry."
    )]
    #[case::env_var(
        RequestOrigin::EnvVar("APOLLO_ROVER_DEV_ROUTER_VERSION"),
        "The `supergraph` plugin v2.9.3, set by `APOLLO_ROVER_DEV_ROUTER_VERSION`, is no longer available from the plugin registry."
    )]
    #[case::supergraph_config(
        RequestOrigin::SupergraphConfig,
        "The `supergraph` plugin v2.9.3, set by `federation_version` in `supergraph.yaml`, is no longer available from the plugin registry."
    )]
    fn a_withdrawn_version_names_where_the_request_came_from(
        #[case] origin: RequestOrigin,
        #[case] expected: &str,
    ) {
        assert_that!(no_longer_served(origin, None).to_string()).is_equal_to(expected.to_string());
    }

    #[rstest]
    #[case::resolution(resolution(), PluginName::Supergraph, "=2.9.9")]
    #[case::download(download(), PluginName::Router, "2")]
    #[case::installation(installation(), PluginName::ApolloMcpServer, "latest")]
    #[case::no_longer_served(
        no_longer_served(RequestOrigin::SupergraphConfig, None),
        PluginName::Supergraph,
        "=2.9.3"
    )]
    fn every_failure_identifies_the_plugin_and_the_request(
        #[case] failure: PluginFailure,
        #[case] plugin: PluginName,
        #[case] requested: &str,
    ) {
        assert_that!((
            failure.plugin(),
            failure.requested().map(|request| request.to_string())
        ))
        .is_equal_to((Some(plugin), Some(requested.to_string())));
    }
}
