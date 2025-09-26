use serde::Serialize;
use std::collections::HashMap;
use std::fmt::{self, Display};
use strum_macros::EnumString;

/// `Code` contains the error codes associated with specific errors.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, EnumString)]
pub enum RoverErrorCode {
    E001,
    E002,
    E003,
    E004,
    E005,
    E006,
    E007,
    E008,
    E009,
    E010,
    E011,
    E012,
    E013,
    E014,
    E015,
    E016,
    E017,
    E018,
    E019,
    E020,
    E021,
    E022,
    E023,
    E024,
    E025,
    E026,
    E027,
    E028,
    E029,
    E030,
    E031,
    E032,
    E033,
    E034,
    E035,
    E036,
    E037,
    E038,
    E039,
    E040,
    E041,
    E042,
    E043,
    E044,
    E045,
}

impl Display for RoverErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", &self)
    }
}

impl RoverErrorCode {
    // builds a Map of every possible code and its explanation, so we can
    // access from the `explain` function
    fn explanations() -> HashMap<RoverErrorCode, String> {
        let contents = vec![
            (
                RoverErrorCode::E001,
                include_str!("./codes/E001.md").to_string(),
            ),
            (
                RoverErrorCode::E002,
                include_str!("./codes/E002.md").to_string(),
            ),
            (
                RoverErrorCode::E003,
                include_str!("./codes/E003.md").to_string(),
            ),
            (
                RoverErrorCode::E004,
                include_str!("./codes/E004.md").to_string(),
            ),
            (
                RoverErrorCode::E005,
                include_str!("./codes/E005.md").to_string(),
            ),
            (
                RoverErrorCode::E006,
                include_str!("./codes/E006.md").to_string(),
            ),
            (
                RoverErrorCode::E007,
                include_str!("./codes/E007.md").to_string(),
            ),
            (
                RoverErrorCode::E008,
                include_str!("./codes/E008.md").to_string(),
            ),
            (
                RoverErrorCode::E009,
                include_str!("./codes/E009.md").to_string(),
            ),
            (
                RoverErrorCode::E010,
                include_str!("./codes/E010.md").to_string(),
            ),
            (
                RoverErrorCode::E011,
                include_str!("./codes/E011.md").to_string(),
            ),
            (
                RoverErrorCode::E012,
                include_str!("./codes/E012.md").to_string(),
            ),
            (
                RoverErrorCode::E013,
                include_str!("./codes/E013.md").to_string(),
            ),
            (
                RoverErrorCode::E014,
                include_str!("./codes/E014.md").to_string(),
            ),
            (
                RoverErrorCode::E015,
                include_str!("./codes/E015.md").to_string(),
            ),
            (
                RoverErrorCode::E016,
                include_str!("./codes/E016.md").to_string(),
            ),
            (
                RoverErrorCode::E017,
                include_str!("./codes/E017.md").to_string(),
            ),
            (
                RoverErrorCode::E018,
                include_str!("./codes/E018.md").to_string(),
            ),
            (
                RoverErrorCode::E019,
                include_str!("./codes/E019.md").to_string(),
            ),
            (
                RoverErrorCode::E020,
                include_str!("./codes/E020.md").to_string(),
            ),
            (
                RoverErrorCode::E021,
                include_str!("./codes/E021.md").to_string(),
            ),
            (
                RoverErrorCode::E022,
                include_str!("./codes/E022.md").to_string(),
            ),
            (
                RoverErrorCode::E023,
                include_str!("./codes/E023.md").to_string(),
            ),
            (
                RoverErrorCode::E024,
                include_str!("./codes/E024.md").to_string(),
            ),
            (
                RoverErrorCode::E025,
                include_str!("./codes/E025.md").to_string(),
            ),
            (
                RoverErrorCode::E026,
                include_str!("./codes/E026.md").to_string(),
            ),
            (
                RoverErrorCode::E027,
                include_str!("./codes/E027.md").to_string(),
            ),
            (
                RoverErrorCode::E028,
                include_str!("./codes/E028.md").to_string(),
            ),
            (
                RoverErrorCode::E029,
                include_str!("./codes/E029.md").to_string(),
            ),
            (
                RoverErrorCode::E030,
                include_str!("./codes/E030.md").to_string(),
            ),
            (
                RoverErrorCode::E031,
                include_str!("./codes/E031.md").to_string(),
            ),
            (
                RoverErrorCode::E032,
                include_str!("./codes/E032.md").to_string(),
            ),
            (
                RoverErrorCode::E033,
                include_str!("./codes/E033.md").to_string(),
            ),
            (
                RoverErrorCode::E034,
                include_str!("./codes/E034.md").to_string(),
            ),
            (
                RoverErrorCode::E035,
                include_str!("./codes/E035.md").to_string(),
            ),
            (
                RoverErrorCode::E036,
                include_str!("./codes/E036.md").to_string(),
            ),
            (
                RoverErrorCode::E037,
                include_str!("./codes/E037.md").to_string(),
            ),
            (
                RoverErrorCode::E038,
                include_str!("./codes/E038.md").to_string(),
            ),
            (
                RoverErrorCode::E039,
                include_str!("./codes/E039.md").to_string(),
            ),
            (
                RoverErrorCode::E040,
                include_str!("./codes/E040.md").to_string(),
            ),
            (
                RoverErrorCode::E041,
                include_str!("./codes/E041.md").to_string(),
            ),
            (
                RoverErrorCode::E042,
                include_str!("./codes/E042.md").to_string(),
            ),
            (
                RoverErrorCode::E043,
                include_str!("./codes/E043.md").to_string(),
            ),
            (
                RoverErrorCode::E044,
                include_str!("./codes/E044.md").to_string(),
            ),
            (
                RoverErrorCode::E045,
                include_str!("./codes/E045.md").to_string(),
            ),
        ];
        contents.into_iter().collect()
    }

    /// For a given error code, returns a markdown string with a given error's
    /// explanation. Explanations are in ./codes
    pub fn explain(&self) -> String {
        let all_explanations = RoverErrorCode::explanations();
        let explanation = all_explanations.get(self);
        if let Some(explanation) = explanation {
            format!("**{}**\n\n{}\n\n", &self, &explanation)
        } else {
            "Explanation not available".to_string()
        }
    }
}
