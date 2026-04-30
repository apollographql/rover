use serde::Serialize;

#[derive(thiserror::Error, Debug)]
#[error("Unsupported extract extension: {}", .0)]
pub struct UnsupportedExtractExtension(String);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub enum ExtractLanguage {
    TypeScript,
    Swift,
    Kotlin,
}

impl ExtractLanguage {
    pub fn from_extension(ext: &str) -> Result<Self, UnsupportedExtractExtension> {
        match ext {
            "ts" | "tsx" => Ok(Self::TypeScript),
            "swift" => Ok(Self::Swift),
            "kt" | "kts" => Ok(Self::Kotlin),
            ext => Err(UnsupportedExtractExtension(ext.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[rstest]
    #[case::ts("ts", ExtractLanguage::TypeScript)]
    #[case::tsx("tsx", ExtractLanguage::TypeScript)]
    #[case::swift("swift", ExtractLanguage::Swift)]
    #[case::kt("kt", ExtractLanguage::Kotlin)]
    #[case::kts("kts", ExtractLanguage::Kotlin)]
    fn supported_extensions_map_to_correct_language(
        #[case] ext: &str,
        #[case] expected: ExtractLanguage,
    ) {
        assert_that!(ExtractLanguage::from_extension(ext))
            .is_ok()
            .is_equal_to(expected);
    }

    #[rstest]
    #[case::js("js")]
    #[case::py("py")]
    #[case::graphql("graphql")]
    #[case::empty("")]
    fn unsupported_extensions_return_error_containing_the_extension(#[case] ext: &str) {
        assert_that!(ExtractLanguage::from_extension(ext))
            .is_err()
            .matches(|e| e.to_string().contains(ext));
    }
}
