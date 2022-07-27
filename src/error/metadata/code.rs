use serde::Serialize;
use std::collections::HashMap;
use std::fmt::{self, Display};
use strum_macros::EnumString;

/// `Code` contains the error codes associated with specific errors.
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, EnumString)]
pub enum Code {
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
}

impl Display for Code {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", &self)
    }
}

impl Code {
    // builds a Map of every possible code and its explanation, so we can
    // access from the `explain` function
    fn explanations() -> HashMap<Code, String> {
        let contents = vec![
            (Code::E001, include_str!("./codes/E001.md").to_string()),
            (Code::E002, include_str!("./codes/E002.md").to_string()),
            (Code::E003, include_str!("./codes/E003.md").to_string()),
            (Code::E004, include_str!("./codes/E004.md").to_string()),
            (Code::E005, include_str!("./codes/E005.md").to_string()),
            (Code::E006, include_str!("./codes/E006.md").to_string()),
            (Code::E007, include_str!("./codes/E007.md").to_string()),
            (Code::E008, include_str!("./codes/E008.md").to_string()),
            (Code::E009, include_str!("./codes/E009.md").to_string()),
            (Code::E010, include_str!("./codes/E010.md").to_string()),
            (Code::E011, include_str!("./codes/E011.md").to_string()),
            (Code::E012, include_str!("./codes/E012.md").to_string()),
            (Code::E013, include_str!("./codes/E013.md").to_string()),
            (Code::E014, include_str!("./codes/E014.md").to_string()),
            (Code::E015, include_str!("./codes/E015.md").to_string()),
            (Code::E016, include_str!("./codes/E016.md").to_string()),
            (Code::E017, include_str!("./codes/E017.md").to_string()),
            (Code::E018, include_str!("./codes/E018.md").to_string()),
            (Code::E019, include_str!("./codes/E019.md").to_string()),
            (Code::E020, include_str!("./codes/E020.md").to_string()),
            (Code::E021, include_str!("./codes/E021.md").to_string()),
            (Code::E022, include_str!("./codes/E022.md").to_string()),
            (Code::E023, include_str!("./codes/E023.md").to_string()),
            (Code::E024, include_str!("./codes/E024.md").to_string()),
            (Code::E025, include_str!("./codes/E025.md").to_string()),
            (Code::E026, include_str!("./codes/E026.md").to_string()),
            (Code::E027, include_str!("./codes/E027.md").to_string()),
            (Code::E028, include_str!("./codes/E028.md").to_string()),
            (Code::E029, include_str!("./codes/E029.md").to_string()),
            (Code::E030, include_str!("./codes/E030.md").to_string()),
            (Code::E031, include_str!("./codes/E031.md").to_string()),
            (Code::E032, include_str!("./codes/E032.md").to_string()),
            (Code::E033, include_str!("./codes/E033.md").to_string()),
            (Code::E034, include_str!("./codes/E034.md").to_string()),
        ];
        contents.into_iter().collect()
    }

    /// For a given error code, returns a markdown string with a given error's
    /// explanation. Explanations are in ./codes
    pub fn explain(&self) -> String {
        let all_explanations = Code::explanations();
        let explanation = all_explanations.get(self);
        if let Some(explanation) = explanation {
            format!("**{}**\n\n{}\n\n", &self, &explanation)
        } else {
            "Explanation not available".to_string()
        }
    }
}
