use std::{fmt::Display, str::FromStr};

use apollo_federation_types::config::FederationVersion;
use semver::Version;
use serde_json::Value;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum SupergraphVersionError {
    #[error("Unsupported Federation version: {}", .version.to_string())]
    UnsupportedFederationVersion { version: SupergraphVersion },
    #[error("Unable to get version: {}", .error)]
    Conversion { error: String },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SupergraphVersion {
    version: Version,
}

impl SupergraphVersion {
    pub fn new(version: Version) -> SupergraphVersion {
        SupergraphVersion { version }
    }
    /// Establishes whether this version supports the `--output` flag
    pub fn supports_output_flag(&self) -> bool {
        self.version >= Version::from_str("2.9.0").unwrap()
    }
}

impl Display for SupergraphVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.version)
    }
}

impl TryFrom<SupergraphVersion> for FederationVersion {
    type Error = SupergraphVersionError;
    fn try_from(supergraph_version: SupergraphVersion) -> Result<Self, Self::Error> {
        match supergraph_version.version.major {
            0 | 1 => Ok(FederationVersion::ExactFedOne(supergraph_version.version)),
            2 => Ok(FederationVersion::ExactFedTwo(supergraph_version.version)),
            _ => Err(SupergraphVersionError::UnsupportedFederationVersion {
                version: supergraph_version,
            }),
        }
    }
}

lazy_static::lazy_static! {
    static ref LATEST_PLUGIN_VERSIONS: Value = serde_json::from_str(include_str!("../../../latest_plugin_versions.json")).expect("could not read latest_plugin_versions.json from the root of the repo, which is needed to supply latest versions to `rover supergraph compsoe`.");

}

/// FederationVersion is the apollo_federation_types's view of the version of federation (ie, the
/// spec and its implementation by Apollo) in use. This can be an exact version or point to the
/// latest of a major version (eg, latest of version 1, latest of version 2). The
/// SupergraphVersion, however, is the version of the supergraph binary. These are synonymous, but
/// different; FederationVersion can be inexact by pointing to the latest of some major version
/// while SupergraphVersion must be exact because we must use an exact version of the binary
///
/// Development note: when we have latest-*, we not only get an exact version, we get an exact
/// version specified in our latest_plugins_versions.json. This version might be different than the
/// actual latest version if we haven't updated that file
impl TryFrom<FederationVersion> for SupergraphVersion {
    type Error = SupergraphVersionError;
    fn try_from(federation_version: FederationVersion) -> Result<Self, Self::Error> {
        let supergraph = LATEST_PLUGIN_VERSIONS["supergraph"]
            .as_object()
            .expect("JSON malformed: top-level `supergraph` should be an object");

        let supergraph_versions = supergraph
            .get("versions")
            .expect("JSON malformed: `supergraph.versions` did not exist");

        match federation_version {
            FederationVersion::LatestFedOne => {
                let latest_federation_one = supergraph_versions
                    .get("latest-0")
                    .expect("JSON malformed: `supergraph.versions.latest-0` did not exist")
                    .as_str()
                    .expect("JSON malformed: `supergraph.versions.latest-0` was not a string")
                    .replace("v", "");

                Ok(SupergraphVersion::new(
                    Version::from_str(&latest_federation_one).map_err(|err| {
                        SupergraphVersionError::Conversion {
                            error: err.to_string(),
                        }
                    })?,
                ))
            }
            FederationVersion::LatestFedTwo => {
                let latest_federation_two = supergraph_versions
                    .get("latest-2")
                    .expect("JSON malformed: `supergraph.versions.latest-2` did not exist")
                    .as_str()
                    .expect("JSON malformed: `supergraph.versions.latest-2` was not a string")
                    .replace("v", "");

                Ok(SupergraphVersion::new(
                    Version::from_str(&latest_federation_two).map_err(|err| {
                        SupergraphVersionError::Conversion {
                            error: err.to_string(),
                        }
                    })?,
                ))
            }
            FederationVersion::ExactFedOne(version) | FederationVersion::ExactFedTwo(version) => {
                Ok(SupergraphVersion::new(version))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::SupergraphVersion;

    fn fed_one() -> Version {
        Version::from_str("1.0.0").unwrap()
    }

    fn fed_two_eight() -> Version {
        Version::from_str("2.8.0").unwrap()
    }

    fn fed_two_nine() -> Version {
        Version::from_str("2.9.0").unwrap()
    }

    fn latest_fed_one() -> Version {
        let supergraph = LATEST_PLUGIN_VERSIONS["supergraph"]
            .as_object()
            .expect("JSON malformed: top-level `supergraph` should be an object");

        let supergraph_versions = supergraph
            .get("versions")
            .expect("JSON malformed: `supergraph.versions` did not exist");

        let latest_federation_one = supergraph_versions
            .get("latest-0")
            .expect("JSON malformed: `supergraph.versions.latest-0` did not exist")
            .as_str()
            .expect("JSON malformed: `supergraph.versions.latest-0` was not a string")
            .replace("v", "");

        Version::from_str(&latest_federation_one).unwrap()
    }

    fn latest_fed_two() -> Version {
        let supergraph = LATEST_PLUGIN_VERSIONS["supergraph"]
            .as_object()
            .expect("JSON malformed: top-level `supergraph` should be an object");

        let supergraph_versions = supergraph
            .get("versions")
            .expect("JSON malformed: `supergraph.versions` did not exist");

        let latest_federation_two = supergraph_versions
            .get("latest-2")
            .expect("JSON malformed: `supergraph.versions.latest-2` did not exist")
            .as_str()
            .expect("JSON malformed: `supergraph.versions.latest-2` was not a string")
            .replace("v", "");

        Version::from_str(&latest_federation_two).unwrap()
    }

    #[rstest]
    #[case::fed_one(fed_one(), false)]
    #[case::fed_one(fed_two_eight(), false)]
    #[case::fed_one(fed_two_nine(), true)]
    #[tokio::test]
    async fn test_supports_output_flag(
        #[case] federation_version: Version,
        #[case] expected_result: bool,
    ) {
        let supergraph_version = SupergraphVersion::new(federation_version);
        assert_that!(supergraph_version.supports_output_flag()).is_equal_to(expected_result);
    }

    #[rstest]
    #[case::exact_fed_one(
        FederationVersion::ExactFedOne(fed_one()),
        Ok(SupergraphVersion::new(fed_one()))
    )]
    #[case::exact_fed_two(
        FederationVersion::ExactFedTwo(fed_two_eight()),
        Ok(SupergraphVersion::new(fed_two_eight()))
    )]
    #[case::latest_fed_one(
        FederationVersion::LatestFedOne,
        Ok(SupergraphVersion::new(latest_fed_one()))
    )]
    #[case::latest_fed_two(
        FederationVersion::LatestFedTwo,
        Ok(SupergraphVersion::new(latest_fed_two()))
    )]
    fn test_tryfrom_fedversion_for_supergraphversion(
        #[case] fed_version: FederationVersion,
        #[case] expected: Result<SupergraphVersion, SupergraphVersionError>,
    ) {
        let supergraph_version = TryInto::<SupergraphVersion>::try_into(fed_version);
        assert_that!(supergraph_version).is_equal_to(expected)
    }

    #[rstest]
    #[case::supported_simple(
        SupergraphVersion::new(fed_one()),
        Some(FederationVersion::ExactFedOne(fed_one()))
    )]
    #[case::supported_complex_semver(
        SupergraphVersion::new(Version::from_str("1.2.3-SNAPSHOT.1234+asdf").unwrap()),
        Some(FederationVersion::ExactFedOne(Version::from_str("1.2.3-SNAPSHOT.1234+asdf").unwrap())),

    )]
    #[case::unsupported(
        SupergraphVersion::new(Version::from_str("3.0.0").unwrap()),
        None,
    )]
    fn test_fed_version_from_supergraph_version(
        #[case] supergraph_version: SupergraphVersion,
        #[case] expected_federation_version: Option<FederationVersion>,
    ) {
        // We expect the conversion to work
        if expected_federation_version.is_some() {
            assert_that!(supergraph_version.try_into())
                .is_ok()
                .is_equal_to(expected_federation_version.unwrap());
        // With None, we don't expect the conversion to work
        } else {
            let conversion: Result<FederationVersion, SupergraphVersionError> =
                supergraph_version.clone().try_into();
            assert_that!(conversion).is_err_containing(
                SupergraphVersionError::UnsupportedFederationVersion {
                    version: supergraph_version,
                },
            )
        }
    }
}
