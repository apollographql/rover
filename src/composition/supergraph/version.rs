use std::{fmt::Display, str::FromStr, sync::Arc};

use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use semver::Version;

#[derive(thiserror::Error, Debug, Clone)]
pub enum SupergraphVersionError {
    #[error("Unsupported Federation version: {}", .version.to_string())]
    UnsupportedFederationVersion { version: SupergraphVersion },
    #[error("Unable to get version: {}", .error)]
    Conversion { error: String },
    #[error("Filename does not exist at the given path")]
    MissingFilename,
    #[error("Semver could not be extracted from the installed path")]
    InvalidVersion {
        #[from]
        source: Arc<semver::Error>,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SupergraphVersion {
    version: Version,
}

impl SupergraphVersion {
    pub const fn new(version: Version) -> SupergraphVersion {
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

impl TryFrom<&Utf8PathBuf> for SupergraphVersion {
    type Error = SupergraphVersionError;
    fn try_from(value: &Utf8PathBuf) -> Result<Self, Self::Error> {
        let file_name = value
            .file_name()
            .ok_or_else(|| SupergraphVersionError::MissingFilename)?;
        let without_exe = file_name.strip_suffix(".exe").unwrap_or(file_name);
        let version = Version::parse(
            without_exe
                .strip_prefix("supergraph-v")
                .unwrap_or(without_exe),
        )
        .map_err(Arc::new)?;
        Ok(SupergraphVersion { version })
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

impl PartialEq<Version> for SupergraphVersion {
    fn eq(&self, other: &Version) -> bool {
        self.version == *other
    }
}

impl PartialOrd<Version> for SupergraphVersion {
    fn partial_cmp(&self, other: &Version) -> Option<std::cmp::Ordering> {
        self.version.partial_cmp(other)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::{SupergraphVersion, *};

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
            assert_that!(conversion).is_err().matches(|err| match err {
                SupergraphVersionError::UnsupportedFederationVersion { version } => {
                    version == &supergraph_version
                }
                _ => false,
            });
        }
    }
}
