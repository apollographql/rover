use std::{
    collections::HashMap,
    fmt::{self, Display},
};

use serde::Serialize;
use sputnik::Outcome;
use strum_macros::{EnumIter, EnumString};

/// `Code` contains the error codes associated with specific errors.
#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, EnumString, EnumIter)]
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
    E046,
    E047,
    E048,
    E049,
    E050,
    E051,
    E052,
    E053,
    E054,
    E055,
    E056,
    E057,
    E058,
    E059,
    E060,
    E061,
    E062,
    E063,
    E064,
    E065,
}

impl Display for RoverErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self)
    }
}

impl RoverErrorCode {
    /// How a command that fails with this code ends, per the table in
    /// `specs/rover-telemetry/error-classification.md`. The match has no
    /// wildcard so a new code can't be added without deciding its outcome.
    pub const fn outcome(&self) -> Outcome {
        match self {
            RoverErrorCode::E002
            | RoverErrorCode::E003
            | RoverErrorCode::E007
            | RoverErrorCode::E008
            | RoverErrorCode::E009
            | RoverErrorCode::E010
            | RoverErrorCode::E013
            | RoverErrorCode::E014
            | RoverErrorCode::E016
            | RoverErrorCode::E017
            | RoverErrorCode::E018
            | RoverErrorCode::E019
            | RoverErrorCode::E020
            | RoverErrorCode::E021
            | RoverErrorCode::E022
            | RoverErrorCode::E023
            | RoverErrorCode::E024
            | RoverErrorCode::E027
            | RoverErrorCode::E029
            | RoverErrorCode::E030
            | RoverErrorCode::E032
            | RoverErrorCode::E033
            | RoverErrorCode::E034
            | RoverErrorCode::E035
            | RoverErrorCode::E036
            | RoverErrorCode::E037
            | RoverErrorCode::E038
            | RoverErrorCode::E039
            | RoverErrorCode::E040
            | RoverErrorCode::E041
            | RoverErrorCode::E042
            | RoverErrorCode::E043
            | RoverErrorCode::E044
            | RoverErrorCode::E046
            | RoverErrorCode::E047
            | RoverErrorCode::E051
            | RoverErrorCode::E052
            | RoverErrorCode::E053
            | RoverErrorCode::E054
            | RoverErrorCode::E055
            | RoverErrorCode::E057
            | RoverErrorCode::E058
            | RoverErrorCode::E059
            | RoverErrorCode::E060
            | RoverErrorCode::E061
            | RoverErrorCode::E062
            | RoverErrorCode::E064 => Outcome::UserError,
            RoverErrorCode::E004
            | RoverErrorCode::E028
            | RoverErrorCode::E031
            | RoverErrorCode::E049 => Outcome::ConnectionError,
            // E012 is a user error for an HTTP 4xx and an internal one for a 5xx
            // (`RoverError::outcome` reads the status when there is one). With
            // no status to tell them apart, it's ambiguous, so it's internal.
            RoverErrorCode::E001
            | RoverErrorCode::E005
            | RoverErrorCode::E006
            | RoverErrorCode::E011
            | RoverErrorCode::E012
            | RoverErrorCode::E015
            | RoverErrorCode::E025
            | RoverErrorCode::E026
            | RoverErrorCode::E045
            | RoverErrorCode::E048
            | RoverErrorCode::E050
            | RoverErrorCode::E056
            | RoverErrorCode::E063
            | RoverErrorCode::E065 => Outcome::InternalError,
        }
    }

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
            (
                RoverErrorCode::E046,
                include_str!("./codes/E046.md").to_string(),
            ),
            (
                RoverErrorCode::E047,
                include_str!("./codes/E047.md").to_string(),
            ),
            (
                RoverErrorCode::E048,
                include_str!("./codes/E048.md").to_string(),
            ),
            (
                RoverErrorCode::E049,
                include_str!("./codes/E049.md").to_string(),
            ),
            (
                RoverErrorCode::E050,
                include_str!("./codes/E050.md").to_string(),
            ),
            (
                RoverErrorCode::E051,
                include_str!("./codes/E051.md").to_string(),
            ),
            (
                RoverErrorCode::E052,
                include_str!("./codes/E052.md").to_string(),
            ),
            (
                RoverErrorCode::E053,
                include_str!("./codes/E053.md").to_string(),
            ),
            (
                RoverErrorCode::E054,
                include_str!("./codes/E054.md").to_string(),
            ),
            (
                RoverErrorCode::E055,
                include_str!("./codes/E055.md").to_string(),
            ),
            (
                RoverErrorCode::E056,
                include_str!("./codes/E056.md").to_string(),
            ),
            (
                RoverErrorCode::E057,
                include_str!("./codes/E057.md").to_string(),
            ),
            (
                RoverErrorCode::E058,
                include_str!("./codes/E058.md").to_string(),
            ),
            (
                RoverErrorCode::E059,
                include_str!("./codes/E059.md").to_string(),
            ),
            (
                RoverErrorCode::E060,
                include_str!("./codes/E060.md").to_string(),
            ),
            (
                RoverErrorCode::E061,
                include_str!("./codes/E061.md").to_string(),
            ),
            (
                RoverErrorCode::E062,
                include_str!("./codes/E062.md").to_string(),
            ),
            (
                RoverErrorCode::E063,
                include_str!("./codes/E063.md").to_string(),
            ),
            (
                RoverErrorCode::E064,
                include_str!("./codes/E064.md").to_string(),
            ),
            (
                RoverErrorCode::E065,
                include_str!("./codes/E065.md").to_string(),
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
            format!("**{}**\n\n{}\n\n", self, explanation)
        } else {
            "Explanation not available".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use speculoos::prelude::*;
    use sputnik::Outcome;
    use strum::IntoEnumIterator;

    use super::RoverErrorCode;

    /// The table `RoverErrorCode::outcome` is written from.
    const CLASSIFICATION: &str =
        include_str!("../../../specs/rover-telemetry/error-classification.md");

    fn outcome_named(name: &str) -> Outcome {
        match name {
            "user_error" => Outcome::UserError,
            "internal_error" => Outcome::InternalError,
            "connection_error" => Outcome::ConnectionError,
            other => panic!("`{other}` is not an outcome"),
        }
    }

    /// The outcome the classification doc's table gives each code it lists, by code.
    ///
    /// A row names one outcome, except a row such as E012's that says the status of an HTTP
    /// response decides. `RoverErrorCode::outcome` has no status to go on, and the rule for an
    /// error it can't classify is `internal_error`, so that is the outcome it must give.
    fn documented_outcomes() -> BTreeMap<String, Outcome> {
        let mut documented = BTreeMap::new();
        // | Code | Cause | Outcome | Basis |
        for row in CLASSIFICATION
            .lines()
            .filter(|line| line.starts_with("| E"))
        {
            let cells = row.split('|').map(str::trim).collect::<Vec<_>>();
            let outcomes = cells[3]
                .split('`')
                .skip(1)
                .step_by(2)
                .map(outcome_named)
                .collect::<Vec<_>>();
            let outcome = match outcomes.as_slice() {
                [outcome] => *outcome,
                [_, _] => Outcome::InternalError,
                _ => panic!("can't read the outcome in the row `{row}`"),
            };
            // One row may cover several codes: `E005, E006`.
            for code in cells[1].split(',') {
                documented.insert(code.trim().to_string(), outcome);
            }
        }
        documented
    }

    #[test]
    fn every_code_has_the_outcome_the_classification_doc_gives_it() {
        let documented = documented_outcomes();
        let actual = RoverErrorCode::iter()
            .map(|code| (code.to_string(), code.outcome()))
            .collect::<BTreeMap<_, _>>();

        let mismatches = documented
            .keys()
            .chain(actual.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|code| documented.get(*code) != actual.get(*code))
            .map(|code| {
                format!(
                    "{code}: the doc says {:?}, the code says {:?}",
                    documented.get(code),
                    actual.get(code)
                )
            })
            .collect::<Vec<_>>();

        assert_that!(mismatches).is_equal_to(Vec::<String>::new());
    }
}
