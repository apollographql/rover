//! A version request in the shared plugin grammar, as the per-plugin version
//! types the installer still takes.
//!
//! The grammar accepts every form for every plugin, but the installer can't
//! yet install every request: no form of it means a `supergraph` or `router`
//! major other than the ones that have shipped. A request with no such form
//! is a resolution failure, raised before anything is downloaded.

use std::sync::Arc;

use apollo_federation_types::config::FederationVersion;

use crate::plugin::{
    error::PluginFailure,
    version::{PluginName, VersionRequest},
};

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
        VersionRequest::Exact(version) if version.major >= 2 => {
            Ok(FederationVersion::ExactFedTwo(version.clone()))
        }
        VersionRequest::Exact(version) => Ok(FederationVersion::ExactFedOne(version.clone())),
        VersionRequest::Latest | VersionRequest::Major(2) => Ok(FederationVersion::LatestFedTwo),
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
    use crate::command::install::Plugin;

    #[rstest]
    #[case::exact("=2.9.3", FederationVersion::ExactFedTwo(Version::new(2, 9, 3)))]
    #[case::exact_federation_one("=0.36.0", FederationVersion::ExactFedOne(Version::new(0, 36, 0)))]
    #[case::latest("latest", FederationVersion::LatestFedTwo)]
    #[case::the_federation_two_major("2", FederationVersion::LatestFedTwo)]
    #[case::a_federation_one_major("1", FederationVersion::LatestFedOne)]
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
    fn a_converted_request_converts_back(#[case] request: &str) {
        let request: VersionRequest = request.parse().unwrap();

        let converted = Plugin::Supergraph(federation_version(&request).unwrap());

        assert_that!(converted.request()).is_equal_to(request);
    }

    #[rstest]
    fn a_major_rover_cannot_install_is_a_resolution_failure() {
        let failure = federation_version(&VersionRequest::Major(3)).unwrap_err();

        assert_that!(rover_std::format_error_chain(&*failure)).is_equal_to(
            "Couldn't resolve a release of the `supergraph` plugin matching `3` from the plugin \
             registry.: this version of Rover can't install a `supergraph` release matching `3`"
                .to_string(),
        );
    }
}
