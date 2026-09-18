//! The version grammar shared by every plugin Rover delegates to.
//!
//! One grammar, accepted identically wherever a plugin version can be written:
//! the manifest, `rover plugin install <name>@<version>`, `supergraph.yaml`'s
//! `federation_version`, and every plugin version flag and environment
//! variable. No plugin accepts a form another rejects.

use std::{fmt, str::FromStr};

use semver::Version;

/// The forms a version request may take, as they appear in an error that has
/// to list them. A `major.minor` build track joins this list in a later
/// release; see [`VersionRequest`].
const ACCEPTED_FORMS: &str =
    "`latest`, a major version such as `2`, or an exact version such as `=2.9.0`";

/// One of exactly three binaries Rover delegates to. No other name is valid
/// anywhere in the plugin contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PluginName {
    Supergraph,
    Router,
    ApolloMcpServer,
}

impl PluginName {
    /// Every plugin, in the order errors and listings should name them.
    pub const ALL: [PluginName; 3] = [Self::Supergraph, Self::Router, Self::ApolloMcpServer];

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Supergraph => "supergraph",
            Self::Router => "router",
            Self::ApolloMcpServer => "apollo-mcp-server",
        }
    }
}

impl fmt::Display for PluginName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A plugin name Rover does not recognize, naming the three that it does.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "`{value}` is not a Rover plugin. Valid plugins are `supergraph`, `router`, and `apollo-mcp-server`."
)]
pub struct UnknownPluginName {
    pub value: String,
}

impl FromStr for PluginName {
    type Err = UnknownPluginName;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|plugin| plugin.as_str() == value)
            .ok_or_else(|| UnknownPluginName {
                value: value.to_string(),
            })
    }
}

/// A version a user asked for, before it has been resolved against the
/// registry. [`Self::Latest`] and [`Self::Major`] are floating: they name a
/// range, and which release they mean depends on when they are resolved.
/// [`Self::Exact`] names one release and needs no resolution.
///
/// A `major.minor` build track — the newest patch within one minor — is a
/// fourth form this type will grow. Prefer the accessors below ([`major`],
/// [`exact`], [`is_floating`]) over matching every variant, so that adding it
/// does not mean revisiting every call site.
///
/// [`major`]: VersionRequest::major
/// [`exact`]: VersionRequest::exact
/// [`is_floating`]: VersionRequest::is_floating
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionRequest {
    /// The newest release of the plugin, any major.
    Latest,
    /// The newest release within one major version.
    Major(u64),
    /// Exactly this release.
    Exact(Version),
}

impl VersionRequest {
    /// Parse a version written for a particular plugin, naming that plugin if
    /// it does not parse. Prefer this over [`FromStr`] wherever the plugin is
    /// known, which is nearly everywhere.
    pub fn parse_for(plugin: PluginName, value: &str) -> Result<Self, InvalidPluginVersion> {
        value.parse().map_err(|_| InvalidPluginVersion {
            plugin,
            value: value.to_string(),
        })
    }

    /// The major version this request is confined to, or `None` for a request
    /// that spans majors.
    pub const fn major(&self) -> Option<u64> {
        match self {
            Self::Latest => None,
            Self::Major(major) => Some(*major),
            Self::Exact(version) => Some(version.major),
        }
    }

    /// The single release this request names, or `None` if resolving it
    /// against the registry is what decides.
    pub const fn exact(&self) -> Option<&Version> {
        match self {
            Self::Exact(version) => Some(version),
            _ => None,
        }
    }

    /// Whether this request must be resolved against the registry to become a
    /// single release.
    pub const fn is_floating(&self) -> bool {
        self.exact().is_none()
    }
}

impl fmt::Display for VersionRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Latest => f.write_str("latest"),
            Self::Major(major) => write!(f, "{major}"),
            Self::Exact(version) => write!(f, "={version}"),
        }
    }
}

impl FromStr for VersionRequest {
    type Err = InvalidVersionForm;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = || InvalidVersionForm {
            value: value.to_string(),
        };

        if value == "latest" {
            return Ok(Self::Latest);
        }

        if let Some(exact) = value.strip_prefix('=') {
            return Version::parse(exact)
                .map(Self::Exact)
                .map_err(|_| invalid());
        }

        // A bare major, spelled canonically. `02` parses as a u64 but is not a
        // version anyone writes, and accepting it would break the round trip
        // between `Display` and this parse.
        match value.parse::<u64>() {
            Ok(major) if major.to_string() == value => Ok(Self::Major(major)),
            _ => Err(invalid()),
        }
    }
}

/// A version that does not match any accepted form, where the plugin it was
/// written for is not known.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{value}` is not a valid plugin version. Accepted forms are {ACCEPTED_FORMS}.")]
pub struct InvalidVersionForm {
    pub value: String,
}

/// A version that does not match any accepted form, named alongside the plugin
/// it was written for.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "`{value}` is not a valid version for the `{plugin}` plugin. Accepted forms are {ACCEPTED_FORMS}."
)]
pub struct InvalidPluginVersion {
    pub plugin: PluginName,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case("latest", VersionRequest::Latest)]
    #[case("0", VersionRequest::Major(0))]
    #[case("1", VersionRequest::Major(1))]
    #[case("2", VersionRequest::Major(2))]
    #[case("=2.9.0", VersionRequest::Exact(Version::new(2, 9, 0)))]
    #[case("=0.36.0", VersionRequest::Exact(Version::new(0, 36, 0)))]
    fn every_accepted_form_parses_the_same_for_every_plugin(
        #[case] input: &str,
        #[case] expected: VersionRequest,
        #[values(
            PluginName::Supergraph,
            PluginName::Router,
            PluginName::ApolloMcpServer
        )]
        plugin: PluginName,
    ) {
        assert_that!(VersionRequest::parse_for(plugin, input))
            .is_ok()
            .is_equal_to(expected);
    }

    #[rstest]
    #[case("=2.0.0-preview.9", "2.0.0-preview.9")]
    #[case("=2.9.0+build.1", "2.9.0+build.1")]
    fn an_exact_version_keeps_its_prerelease_and_build_metadata(
        #[case] input: &str,
        #[case] expected: &str,
    ) {
        let expected = VersionRequest::Exact(Version::parse(expected).unwrap());

        assert_that!(input.parse::<VersionRequest>())
            .is_ok()
            .is_equal_to(expected);
    }

    #[rstest]
    #[case(VersionRequest::Latest, "latest")]
    #[case(VersionRequest::Major(2), "2")]
    #[case(VersionRequest::Exact(Version::new(2, 9, 0)), "=2.9.0")]
    #[case(
        VersionRequest::Exact(Version::parse("2.0.0-preview.9").unwrap()),
        "=2.0.0-preview.9"
    )]
    fn display_round_trips_through_parsing(
        #[case] request: VersionRequest,
        #[case] expected: &str,
    ) {
        assert_that!(request.to_string().as_str()).is_equal_to(expected);
        assert_that!(expected.parse::<VersionRequest>())
            .is_ok()
            .is_equal_to(request);
    }

    #[rstest]
    // A build track. Accepted in a later release, not yet.
    #[case("2.9")]
    // Not canonically spelled, so it would not survive a round trip.
    #[case("02")]
    #[case(" 2")]
    #[case("2 ")]
    // An exact version has to say so with an `=`.
    #[case("2.9.0")]
    // The legacy forms, which a later PR in this stack accepts.
    #[case("latest-2")]
    #[case("v2.9.0")]
    #[case("")]
    #[case("latest2")]
    #[case("=")]
    #[case("=not-a-version")]
    #[case("newest")]
    fn an_unaccepted_form_is_rejected_naming_the_plugin_and_the_accepted_forms(
        #[case] input: &str,
    ) {
        let error = VersionRequest::parse_for(PluginName::Router, input)
            .expect_err("should not have parsed");

        assert_that!(error.to_string().as_str()).is_equal_to(
            format!(
                "`{input}` is not a valid version for the `router` plugin. Accepted forms are \
                 `latest`, a major version such as `2`, or an exact version such as `=2.9.0`."
            )
            .as_str(),
        );
    }

    #[rstest]
    #[case("0", VersionRequest::Major(0))]
    #[case("1", VersionRequest::Major(1))]
    #[case("=0.36.0", VersionRequest::Exact(Version::new(0, 36, 0)))]
    #[case("=1.2.3", VersionRequest::Exact(Version::new(1, 2, 3)))]
    fn a_federation_one_version_parses_and_is_rejected_elsewhere(
        #[case] input: &str,
        #[case] expected: VersionRequest,
    ) {
        // Federation 1 is refused on what the version means, not on how it is
        // written, so the grammar has to accept these and leave the refusal to
        // the composition path.
        assert_that!(VersionRequest::parse_for(PluginName::Supergraph, input))
            .is_ok()
            .is_equal_to(expected);
    }

    #[rstest]
    #[case(VersionRequest::Latest, None, true)]
    #[case(VersionRequest::Major(2), Some(2), true)]
    #[case(VersionRequest::Exact(Version::new(2, 9, 0)), Some(2), false)]
    fn accessors_describe_what_still_needs_resolving(
        #[case] request: VersionRequest,
        #[case] expected_major: Option<u64>,
        #[case] expected_floating: bool,
    ) {
        assert_that!(request.major()).is_equal_to(expected_major);
        assert_that!(request.is_floating()).is_equal_to(expected_floating);
        assert_that!(request.exact().is_none()).is_equal_to(expected_floating);
    }

    #[rstest]
    #[case(PluginName::Supergraph, "supergraph")]
    #[case(PluginName::Router, "router")]
    #[case(PluginName::ApolloMcpServer, "apollo-mcp-server")]
    fn a_plugin_name_round_trips(#[case] plugin: PluginName, #[case] expected: &str) {
        assert_that!(plugin.to_string().as_str()).is_equal_to(expected);
        assert_that!(expected.parse::<PluginName>())
            .is_ok()
            .is_equal_to(plugin);
    }

    #[rstest]
    #[case("Supergraph")]
    #[case("mcp")]
    #[case("apollo-router")]
    #[case("")]
    fn an_unknown_plugin_name_is_rejected_naming_the_three_valid_ones(#[case] input: &str) {
        let error = input
            .parse::<PluginName>()
            .expect_err("should not have parsed");

        assert_that!(error.to_string().as_str()).is_equal_to(
            format!(
                "`{input}` is not a Rover plugin. Valid plugins are `supergraph`, `router`, and \
                 `apollo-mcp-server`."
            )
            .as_str(),
        );
    }
}
