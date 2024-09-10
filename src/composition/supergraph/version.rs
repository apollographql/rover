use std::{fmt::Display, str::FromStr};

use apollo_federation_types::config::FederationVersion;
use semver::Version;

#[derive(thiserror::Error, Debug, PartialEq)]
pub enum SupergraphVersionError {
    #[error("Unsupported Federation version: {}", .version.to_string())]
    UnsupportedFederationVersion { version: SupergraphVersion },
}

#[derive(Debug, Clone, PartialEq)]
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
        write!(f, "{}", self.version.to_string())
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
