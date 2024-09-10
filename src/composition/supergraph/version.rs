use std::str::FromStr;

use semver::Version;

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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

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
}
