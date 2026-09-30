//! The error contract for plugin failures.
//!
//! Each class of plugin failure has its own variant of [`PluginFailure`]:
//! resolution, download, installation, a withdrawn release, and a manifest
//! or lockfile that can't be used so far, with the never-download and
//! checksum classes to follow. Every variant has its own stable [`RoverErrorCode`] and a
//! [`PluginNextStep`] naming the plugin, or for a manifest or lockfile the
//! file to change, and what to do about it. A [`PluginFailure`] anywhere in an error's cause
//! chain decides that error's code and suggestion, so a caller that wraps one
//! keeps both.
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

/// The [`PluginFailure`] behind `error`, found anywhere in its cause chain,
/// boxed or not: an error type that carries one boxes it to stay small.
pub fn find_in_chain(error: &anyhow::Error) -> Option<&PluginFailure> {
    error.chain().find_map(|cause| {
        cause.downcast_ref::<PluginFailure>().or_else(|| {
            cause
                .downcast_ref::<Box<PluginFailure>>()
                .map(|boxed| &**boxed)
        })
    })
}

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

    /// A plugin manifest exists but cannot be used. It names no single
    /// plugin, even when one entry is what is wrong: the problem is the file,
    /// and fixing the file is the next step.
    Manifest {
        path: Utf8PathBuf,
        problem: ManifestProblem,
    },

    /// A plugin lockfile exists but cannot be used. Like [`Self::Manifest`],
    /// it names the file rather than any one plugin.
    Lockfile {
        path: Utf8PathBuf,
        problem: LockfileProblem,
    },
}

/// A manifest's or lockfile's bytes that are not UTF-8 text.
#[derive(Debug, thiserror::Error)]
#[error("not UTF-8 text")]
pub(crate) struct NotUtf8(#[source] pub(crate) std::string::FromUtf8Error);

/// Why a manifest cannot be used.
#[derive(Debug, Clone)]
pub enum ManifestProblem {
    /// The file is there but could not be read.
    Unreadable(PluginFailureCause),
    /// The file is not valid YAML, does not have a manifest's shape, or
    /// is not text at all; the cause says which.
    Malformed(PluginFailureCause),
    /// The file sets `install_root`, which this version of Rover does not
    /// honor. Ignoring it would install binaries somewhere the manifest did
    /// not ask for, and move them once a later version starts honoring it.
    UnsupportedInstallRoot,
}

/// Why a lockfile cannot be used.
#[derive(Debug, Clone)]
pub enum LockfileProblem {
    /// The file is there but could not be read.
    Unreadable(PluginFailureCause),
    /// The file is not valid TOML, does not have a lockfile's shape, or is
    /// not text at all; the cause says which.
    Malformed(PluginFailureCause),
    /// The file could not be written, after the install or removal it
    /// records had succeeded.
    Unwritable(PluginFailureCause),
    /// The file is in a newer format than this version of Rover reads.
    /// Ignoring it would install whatever resolves today, and overwriting it
    /// would lose what the newer Rover recorded.
    WrittenByNewerRover { format_version: u64 },
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
            Self::Manifest { path, problem } => match problem {
                ManifestProblem::Unreadable(_) => write!(f, "Couldn't read the manifest `{path}`."),
                ManifestProblem::Malformed(_) => write!(f, "`{path}` is not a valid manifest."),
                ManifestProblem::UnsupportedInstallRoot => write!(
                    f,
                    "`{path}` sets `install_root`, which this version of Rover doesn't support."
                ),
            },
            Self::Lockfile { path, problem } => match problem {
                LockfileProblem::Unreadable(_) => {
                    write!(f, "Couldn't read the plugin lockfile `{path}`.")
                }
                LockfileProblem::Malformed(_) => {
                    write!(f, "`{path}` is not a valid plugin lockfile.")
                }
                LockfileProblem::Unwritable(_) => {
                    write!(f, "Couldn't write the plugin lockfile `{path}`.")
                }
                LockfileProblem::WrittenByNewerRover { format_version } => write!(
                    f,
                    "`{path}` was written by a newer version of Rover, in lockfile format version {format_version}."
                ),
            },
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
            | Self::Installation { source, .. }
            | Self::Manifest {
                problem: ManifestProblem::Unreadable(source) | ManifestProblem::Malformed(source),
                ..
            }
            | Self::Lockfile {
                problem:
                    LockfileProblem::Unreadable(source)
                    | LockfileProblem::Malformed(source)
                    | LockfileProblem::Unwritable(source),
                ..
            } => Some(&**source),
            Self::NoLongerServed { .. }
            | Self::Manifest {
                problem: ManifestProblem::UnsupportedInstallRoot,
                ..
            }
            | Self::Lockfile {
                problem: LockfileProblem::WrittenByNewerRover { .. },
                ..
            } => None,
        }
    }
}

impl PluginFailure {
    /// The plugin that could not be obtained.
    ///
    /// `None` for a failure that names no single plugin, such as a malformed
    /// manifest, which is about a file rather than any one plugin.
    pub const fn plugin(&self) -> Option<PluginName> {
        match self {
            Self::Resolution { plugin, .. }
            | Self::Download { plugin, .. }
            | Self::Installation { plugin, .. }
            | Self::NoLongerServed { plugin, .. } => Some(*plugin),
            Self::Manifest { .. } | Self::Lockfile { .. } => None,
        }
    }

    /// The version as it was asked for, before any resolution.
    ///
    /// `None` for a failure that names no request, such as a malformed
    /// manifest, which is about a file rather than any one plugin.
    pub fn requested(&self) -> Option<VersionRequest> {
        match self {
            Self::Resolution { requested, .. }
            | Self::Download { requested, .. }
            | Self::Installation { requested, .. } => Some(requested.clone()),
            Self::NoLongerServed { version, .. } => Some(VersionRequest::Exact(version.clone())),
            Self::Manifest { .. } | Self::Lockfile { .. } => None,
        }
    }

    /// The stable code this failure is reported under.
    pub const fn code(&self) -> RoverErrorCode {
        match self {
            Self::Resolution { .. } => RoverErrorCode::E048,
            Self::Download { .. } => RoverErrorCode::E049,
            Self::Installation { .. } => RoverErrorCode::E050,
            Self::NoLongerServed { .. } => RoverErrorCode::E051,
            Self::Manifest { .. } | Self::Lockfile { .. } => RoverErrorCode::E052,
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
            Self::Manifest { path, problem } => {
                let path = path.clone();
                match problem {
                    ManifestProblem::Unreadable(_) => PluginNextStep::MakeReadable { path },
                    ManifestProblem::Malformed(_) => PluginNextStep::FixFile { path },
                    ManifestProblem::UnsupportedInstallRoot => {
                        PluginNextStep::RemoveInstallRoot { path }
                    }
                }
            }
            Self::Lockfile { path, problem } => {
                let path = path.clone();
                match problem {
                    LockfileProblem::Unreadable(_) => PluginNextStep::MakeReadable { path },
                    LockfileProblem::Malformed(_) => PluginNextStep::FixFile { path },
                    LockfileProblem::Unwritable(_) => PluginNextStep::MakeWritable { path },
                    LockfileProblem::WrittenByNewerRover { .. } => {
                        PluginNextStep::UpgradeRover { path }
                    }
                }
            }
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
    /// `rover plugin install <name>@<version>`, or its deprecated alias
    /// `rover install --plugin`.
    PluginArgument,
    /// A version flag such as `--federation-version`.
    Flag(&'static str),
    /// An environment variable such as `APOLLO_ROVER_DEV_ROUTER_VERSION`.
    EnvVar(&'static str),
    /// `federation_version` in the supergraph config, at this path when it
    /// was read from a file.
    SupergraphConfig(Option<Utf8PathBuf>),
}

impl RequestOrigin {
    /// Where a value clap took from `flag`, or from `env` when the flag is
    /// absent, came from. Clap folds the two into one value without saying
    /// which supplied it, and the flag wins when both are set, so the variable
    /// is the origin only when the flag isn't on the command line.
    pub(crate) fn flag_or_env(flag: &'static str, env: &'static str) -> Self {
        let with_value = format!("{flag}=");
        let flag_given = std::env::args().any(|arg| arg == flag || arg.starts_with(&with_value));
        if !flag_given && std::env::var_os(env).is_some() {
            Self::EnvVar(env)
        } else {
            Self::Flag(flag)
        }
    }

    /// The supergraph config, as a sentence names it.
    fn supergraph_config(path: Option<&Utf8PathBuf>) -> String {
        path.map_or_else(
            || "the supergraph config".to_string(),
            |path| format!("`{path}`"),
        )
    }
}

impl fmt::Display for RequestOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PluginArgument => f.write_str("requested by `rover plugin install`"),
            Self::Flag(flag) => write!(f, "requested by `{flag}`"),
            Self::EnvVar(var) => write!(f, "set by `{var}`"),
            Self::SupergraphConfig(path) => write!(
                f,
                "set by `federation_version` in {}",
                Self::supergraph_config(path.as_ref())
            ),
        }
    }
}

/// The concrete next step a [`PluginFailure`] suggests. Each names the plugin,
/// or for a manifest or lockfile that can't be used, the file to change.
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
    MakeReadable {
        path: Utf8PathBuf,
    },
    FixFile {
        path: Utf8PathBuf,
    },
    RemoveInstallRoot {
        path: Utf8PathBuf,
    },
    UpgradeRover {
        path: Utf8PathBuf,
    },
    MakeWritable {
        path: Utf8PathBuf,
    },
}

impl fmt::Display for PluginNextStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CheckRegistry { plugin, requested } => write!(
                f,
                "Make sure the plugin registry is reachable and that `{plugin}` has a release matching `{requested}`, then re-run the command. If you use a registry other than Apollo's, check that `--download-host` (or `APOLLO_ROVER_DOWNLOAD_HOST`) points at it."
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
                "Make sure `{install_root}` is writable and has free space, then run `rover plugin install {plugin}@={version} --force --elv2-license accept` to reinstall it."
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
                    RequestOrigin::PluginArgument => {
                        write!(f, "Run `rover plugin install {plugin}@{request}`{newest}.")
                    }
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
                    RequestOrigin::SupergraphConfig(path) => write!(
                        f,
                        "Set `federation_version: {request}` in {}{newest}.",
                        RequestOrigin::supergraph_config(path.as_ref())
                    ),
                }
            }
            Self::MakeReadable { path } => write!(
                f,
                "Make sure `{path}` is a file you can read, then re-run the command."
            ),
            Self::FixFile { path } => {
                write!(f, "Fix `{path}`, or remove it, then re-run the command.")
            }
            Self::RemoveInstallRoot { path } => write!(
                f,
                "Remove `install_root` from `{path}`. Plugins install into the `bin` directory next to it."
            ),
            Self::MakeWritable { path } => write!(
                f,
                "Make sure you can write to `{path}` and the directory holding it, then re-run the command."
            ),
            Self::UpgradeRover { path } => write!(
                f,
                "Upgrade Rover to a version that reads `{path}`, then re-run the command. This version won't read the file or overwrite it."
            ),
        }
    }
}

/// The error exactly as `rover` prints it, uncoloured.
///
/// With `RUST_BACKTRACE` set, as it is in CI, anyhow appends a
/// `Stack backtrace:` block after the causes. That comes from the
/// environment, not the failure, so it's cut out here. The block runs up to
/// the first suggestion, which is the only line indented by exactly eight
/// spaces: frame lines are indented less, and their `at` lines more.
#[cfg(test)]
pub(crate) fn printed(error: impl Into<anyhow::Error>) -> String {
    let printed =
        console::strip_ansi_codes(&crate::RoverError::new(error).to_string()).into_owned();
    let Some(start) = printed.find("\n\nStack backtrace:\n") else {
        return printed;
    };
    let is_suggestion = |line: &&str| {
        line.strip_prefix("        ")
            .is_some_and(|text| !text.starts_with(' '))
    };
    let suggestions = printed[start + 2..]
        .split_inclusive('\n')
        .skip_while(|line| !is_suggestion(line))
        .collect::<String>();
    format!("{}\n{suggestions}", &printed[..start])
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

    fn manifest(problem: ManifestProblem) -> PluginFailure {
        PluginFailure::Manifest {
            path: Utf8PathBuf::from("/work/app/.rover/rover.yaml"),
            problem,
        }
    }

    fn lockfile(problem: LockfileProblem) -> PluginFailure {
        PluginFailure::Lockfile {
            path: Utf8PathBuf::from("/work/app/.rover/plugin-versions.lock"),
            problem,
        }
    }

    fn newer_lockfile() -> PluginFailure {
        lockfile(LockfileProblem::WrittenByNewerRover { format_version: 2 })
    }

    fn malformed_manifest() -> PluginFailure {
        manifest(ManifestProblem::Malformed(cause(
            "plugins: invalid type: sequence, expected a mapping of plugin name to version at line 2 column 3",
        )))
    }

    fn unreadable_manifest() -> PluginFailure {
        manifest(ManifestProblem::Unreadable(cause(
            "Permission denied (os error 13)",
        )))
    }

    #[rstest]
    #[case::resolution(
        resolution(),
        "error[E048]: Couldn't resolve a release of the `supergraph` plugin matching `=2.9.9` from the plugin registry.\n\
         \n\
         Caused by:\n    \
         Bad Status code: 404 Not Found\n        \
         Make sure the plugin registry is reachable and that `supergraph` has a release matching `=2.9.9`, then re-run the command. If you use a registry other than Apollo's, check that `--download-host` (or `APOLLO_ROVER_DOWNLOAD_HOST`) points at it.\n"
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
         Make sure `/home/me/.rover/bin` is writable and has free space, then run `rover plugin install apollo-mcp-server@=1.0.0 --force --elv2-license accept` to reinstall it.\n"
    )]
    #[case::no_longer_served(
        no_longer_served(RequestOrigin::PluginArgument, Some("2.9.5")),
        "error[E051]: The `supergraph` plugin v2.9.3, requested by `rover plugin install`, is no longer available from the plugin registry. The newest available 2.x is v2.9.5.\n        \
         Run `rover plugin install supergraph@=2.9.5`.\n"
    )]
    #[case::malformed_manifest(
        malformed_manifest(),
        "error[E052]: `/work/app/.rover/rover.yaml` is not a valid manifest.\n\
         \n\
         Caused by:\n    \
         plugins: invalid type: sequence, expected a mapping of plugin name to version at line 2 column 3\n        \
         Fix `/work/app/.rover/rover.yaml`, or remove it, then re-run the command.\n"
    )]
    #[case::unreadable_manifest(
        unreadable_manifest(),
        "error[E052]: Couldn't read the manifest `/work/app/.rover/rover.yaml`.\n\
         \n\
         Caused by:\n    \
         Permission denied (os error 13)\n        \
         Make sure `/work/app/.rover/rover.yaml` is a file you can read, then re-run the command.\n"
    )]
    #[case::unsupported_install_root(
        manifest(ManifestProblem::UnsupportedInstallRoot),
        "error[E052]: `/work/app/.rover/rover.yaml` sets `install_root`, which this version of Rover doesn't support.\n        \
         Remove `install_root` from `/work/app/.rover/rover.yaml`. Plugins install into the `bin` directory next to it.\n"
    )]
    #[case::malformed_lockfile(
        lockfile(LockfileProblem::Malformed(cause("missing field `version`"))),
        "error[E052]: `/work/app/.rover/plugin-versions.lock` is not a valid plugin lockfile.\n\
         \n\
         Caused by:\n    \
         missing field `version`\n        \
         Fix `/work/app/.rover/plugin-versions.lock`, or remove it, then re-run the command.\n"
    )]
    #[case::unreadable_lockfile(
        lockfile(LockfileProblem::Unreadable(cause("Permission denied (os error 13)"))),
        "error[E052]: Couldn't read the plugin lockfile `/work/app/.rover/plugin-versions.lock`.\n\
         \n\
         Caused by:\n    \
         Permission denied (os error 13)\n        \
         Make sure `/work/app/.rover/plugin-versions.lock` is a file you can read, then re-run the command.\n"
    )]
    #[case::unwritable_lockfile(
        lockfile(LockfileProblem::Unwritable(cause("Read-only file system (os error 30)"))),
        "error[E052]: Couldn't write the plugin lockfile `/work/app/.rover/plugin-versions.lock`.\n\
         \n\
         Caused by:\n    \
         Read-only file system (os error 30)\n        \
         Make sure you can write to `/work/app/.rover/plugin-versions.lock` and the directory holding it, then re-run the command.\n"
    )]
    #[case::newer_lockfile(
        newer_lockfile(),
        "error[E052]: `/work/app/.rover/plugin-versions.lock` was written by a newer version of Rover, in lockfile format version 2.\n        \
         Upgrade Rover to a version that reads `/work/app/.rover/plugin-versions.lock`, then re-run the command. This version won't read the file or overwrite it.\n"
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
            "message": "The `supergraph` plugin v2.9.3, requested by `rover plugin install`, is no longer available from the plugin registry.",
            "code": "E051",
        })
    )]
    #[case::malformed_manifest(
        malformed_manifest(),
        serde_json::json!({
            "message": "`/work/app/.rover/rover.yaml` is not a valid manifest.",
            "causes": ["plugins: invalid type: sequence, expected a mapping of plugin name to version at line 2 column 3"],
            "code": "E052",
        })
    )]
    #[case::unreadable_manifest(
        unreadable_manifest(),
        serde_json::json!({
            "message": "Couldn't read the manifest `/work/app/.rover/rover.yaml`.",
            "causes": ["Permission denied (os error 13)"],
            "code": "E052",
        })
    )]
    #[case::unsupported_install_root(
        manifest(ManifestProblem::UnsupportedInstallRoot),
        serde_json::json!({
            "message": "`/work/app/.rover/rover.yaml` sets `install_root`, which this version of Rover doesn't support.",
            "code": "E052",
        })
    )]
    #[case::newer_lockfile(
        newer_lockfile(),
        serde_json::json!({
            "message": "`/work/app/.rover/plugin-versions.lock` was written by a newer version of Rover, in lockfile format version 2.",
            "code": "E052",
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
    #[case::manifest(malformed_manifest(), include_str!("../error/metadata/codes/E052.md"))]
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
        "Run `rover plugin install supergraph@=2.9.5`."
    )]
    #[case::argument_without_listing(
        RequestOrigin::PluginArgument,
        None,
        "Run `rover plugin install supergraph@2` to use the newest available 2.x."
    )]
    #[case::flag(
        RequestOrigin::Flag("--federation-version"),
        Some("2.9.5"),
        "Re-run with `--federation-version =2.9.5`."
    )]
    #[case::the_newest_is_the_withdrawn_release(
        RequestOrigin::PluginArgument,
        Some("2.9.3"),
        "Run `rover plugin install supergraph@2` to use the newest available 2.x."
    )]
    #[case::the_newest_is_in_another_major(
        RequestOrigin::PluginArgument,
        Some("3.0.0"),
        "Run `rover plugin install supergraph@2` to use the newest available 2.x."
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
        RequestOrigin::SupergraphConfig(Some(Utf8PathBuf::from("graphs/prod.yaml"))),
        Some("2.9.5"),
        "Set `federation_version: =2.9.5` in `graphs/prod.yaml`."
    )]
    #[case::supergraph_config_from_stdin(
        RequestOrigin::SupergraphConfig(None),
        Some("2.9.5"),
        "Set `federation_version: =2.9.5` in the supergraph config."
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
                "The `supergraph` plugin v2.9.3, requested by `rover plugin install`, is no longer available from the plugin registry."
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
            "Run `rover plugin install apollo-mcp-server@latest` to use the newest available release, which may be a newer major version."
                .to_string(),
        );
    }

    #[rstest]
    #[case::argument(
        RequestOrigin::PluginArgument,
        "The `supergraph` plugin v2.9.3, requested by `rover plugin install`, is no longer available from the plugin registry."
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
        RequestOrigin::SupergraphConfig(Some(Utf8PathBuf::from("graphs/prod.yaml"))),
        "The `supergraph` plugin v2.9.3, set by `federation_version` in `graphs/prod.yaml`, is no longer available from the plugin registry."
    )]
    #[case::supergraph_config_from_stdin(
        RequestOrigin::SupergraphConfig(None),
        "The `supergraph` plugin v2.9.3, set by `federation_version` in the supergraph config, is no longer available from the plugin registry."
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
        no_longer_served(RequestOrigin::SupergraphConfig(None), None),
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

    /// A manifest or lockfile failure is about a file, not a plugin, so it
    /// names neither a plugin nor a request, even when the file's one bad
    /// entry names both.
    #[rstest]
    #[case::malformed(malformed_manifest())]
    #[case::unreadable(unreadable_manifest())]
    #[case::unsupported_install_root(manifest(ManifestProblem::UnsupportedInstallRoot))]
    #[case::newer_lockfile(newer_lockfile())]
    fn a_file_failure_names_no_plugin_or_request(#[case] failure: PluginFailure) {
        assert_that!((failure.plugin(), failure.requested())).is_equal_to((None, None));
    }
}
