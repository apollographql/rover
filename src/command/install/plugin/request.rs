//! A version request in the shared plugin grammar, as the per-plugin version
//! types the installer still takes.
//!
//! The grammar accepts every form for every plugin, but the installer can't
//! yet install every request: no form of it means a `supergraph` or `router`
//! major other than the ones that have shipped. A request with no such form
//! is a resolution failure, raised before anything is downloaded.

use std::sync::Arc;

use apollo_federation_types::config::{FederationVersion, RouterVersion};

use super::{Plugin, mcp};
use crate::plugin::{
    error::PluginFailure,
    version::{PluginName, VersionRequest},
};

impl Plugin {
    /// `plugin` at `request`, or the resolution failure that says this
    /// version of Rover can't install a release `request` allows.
    pub fn from_request(
        plugin: PluginName,
        request: &VersionRequest,
    ) -> Result<Self, Box<PluginFailure>> {
        let converted = match plugin {
            PluginName::Supergraph => return federation_version(request).map(Self::Supergraph),
            PluginName::Router => match request {
                VersionRequest::Exact(version) => Some(RouterVersion::Exact(version.clone())),
                VersionRequest::Major(1) => Some(RouterVersion::LatestOne),
                VersionRequest::Latest | VersionRequest::Major(2) => Some(RouterVersion::LatestTwo),
                VersionRequest::Major(3) => Some(RouterVersion::LatestThree),
                VersionRequest::Major(_) => None,
            }
            .map(Self::Router),
            // Its installer has no notion of a major: only `latest`, which
            // could cross out of the one asked for.
            PluginName::ApolloMcpServer => match request {
                VersionRequest::Exact(version) => Some(mcp::Version::Exact(version.clone())),
                VersionRequest::Latest => Some(mcp::Version::Latest),
                VersionRequest::Major(_) => None,
            }
            .map(Self::McpServer),
        };
        converted.ok_or_else(|| uninstallable(plugin, request))
    }
}

/// Why a request the grammar accepts can't be installed.
#[derive(Debug, thiserror::Error)]
#[error("this version of Rover can't install a `{plugin}` release matching `{request}`")]
struct Uninstallable {
    plugin: PluginName,
    request: VersionRequest,
}

fn uninstallable(plugin: PluginName, request: &VersionRequest) -> Box<PluginFailure> {
    Box::new(PluginFailure::Resolution {
        plugin,
        requested: request.clone(),
        source: Arc::new(Uninstallable {
            plugin,
            request: request.clone(),
        }),
    })
}

/// The Federation version `request` asks the `supergraph` plugin for. A
/// Federation 1 request converts, and is refused by the Federation 1 check
/// rather than here.
pub(crate) fn federation_version(
    request: &VersionRequest,
) -> Result<FederationVersion, Box<PluginFailure>> {
    match request {
        VersionRequest::Exact(version) if version.major >= 3 => {
            Ok(FederationVersion::ExactFedThree(version.clone()))
        }
        VersionRequest::Exact(version) if version.major == 2 => {
            Ok(FederationVersion::ExactFedTwo(version.clone()))
        }
        VersionRequest::Exact(version) => Ok(FederationVersion::ExactFedOne(version.clone())),
        VersionRequest::Latest | VersionRequest::Major(2) => Ok(FederationVersion::LatestFedTwo),
        VersionRequest::Major(3) => Ok(FederationVersion::LatestFedThree),
        VersionRequest::Major(0 | 1) => Ok(FederationVersion::LatestFedOne),
        VersionRequest::Major(_) => Err(uninstallable(PluginName::Supergraph, request)),
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case::exact("=2.9.3", FederationVersion::ExactFedTwo(Version::new(2, 9, 3)))]
    #[case::exact_federation_one("=0.36.0", FederationVersion::ExactFedOne(Version::new(0, 36, 0)))]
    #[case::latest("latest", FederationVersion::LatestFedTwo)]
    #[case::the_federation_two_major("2", FederationVersion::LatestFedTwo)]
    #[case::a_federation_one_major("1", FederationVersion::LatestFedOne)]
    #[case::exact_federation_three(
        "=3.0.0-preview.1",
        FederationVersion::ExactFedThree("3.0.0-preview.1".parse().unwrap())
    )]
    #[case::the_federation_three_major("3", FederationVersion::LatestFedThree)]
    fn a_supergraph_request_converts(#[case] request: &str, #[case] expected: FederationVersion) {
        let request: VersionRequest = request.parse().unwrap();

        assert_that!(federation_version(&request).unwrap()).is_equal_to(expected);
    }

    /// Converting back gives the request, apart from `latest`, which means
    /// the newest Federation 2 release: there is no other.
    #[rstest]
    #[case("=2.9.3")]
    #[case("=0.36.0")]
    #[case("2")]
    #[case("3")]
    #[case("=3.0.0-preview.1")]
    fn a_converted_request_converts_back(#[case] request: &str) {
        let request: VersionRequest = request.parse().unwrap();

        let converted = Plugin::Supergraph(federation_version(&request).unwrap());

        assert_that!(converted.request()).is_equal_to(request);
    }

    /// Every request the grammar has for each plugin converts to one that
    /// asks for the same thing, or is refused by name.
    #[rstest]
    #[case::router_exact(PluginName::Router, "=2.1.0", Some("=2.1.0"))]
    #[case::router_latest(PluginName::Router, "latest", Some("2"))]
    #[case::router_two(PluginName::Router, "2", Some("2"))]
    #[case::router_one(PluginName::Router, "1", Some("1"))]
    #[case::router_three(PluginName::Router, "3", Some("3"))]
    #[case::router_four(PluginName::Router, "4", None)]
    #[case::mcp_exact(PluginName::ApolloMcpServer, "=1.0.0", Some("=1.0.0"))]
    #[case::mcp_latest(PluginName::ApolloMcpServer, "latest", Some("latest"))]
    #[case::mcp_major(PluginName::ApolloMcpServer, "1", None)]
    #[case::supergraph_two(PluginName::Supergraph, "2", Some("2"))]
    #[case::supergraph_three(PluginName::Supergraph, "3", Some("3"))]
    #[case::supergraph_four(PluginName::Supergraph, "4", None)]
    fn each_plugin_converts_what_it_can_install(
        #[case] plugin: PluginName,
        #[case] request: &str,
        #[case] expected: Option<&str>,
    ) {
        let request: VersionRequest = request.parse().unwrap();

        let converted = Plugin::from_request(plugin, &request)
            .map(|converted| (converted.name(), converted.request().to_string()))
            .map_err(|failure| failure.to_string());

        assert_that!(converted).is_equal_to(match expected {
            Some(expected) => Ok((plugin, expected.to_string())),
            None => Err(format!(
                "Couldn't resolve a release of the `{plugin}` plugin matching `{request}` from \
                 the plugin registry."
            )),
        });
    }

    #[rstest]
    fn a_major_rover_cannot_install_is_a_resolution_failure() {
        let failure = federation_version(&VersionRequest::Major(4)).unwrap_err();

        assert_that!(rover_std::format_error_chain(&*failure)).is_equal_to(
            "Couldn't resolve a release of the `supergraph` plugin matching `4` from the plugin \
             registry.: this version of Rover can't install a `supergraph` release matching `4`"
                .to_string(),
        );
    }
}
