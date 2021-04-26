use std::fmt::{self, Display};
use crate::anyhow;
use std::collections::BTreeMap;
use std::iter::FromIterator;
use std::str::FromStr;
use serde::Serialize;
/// `Code` contains the error codes associated with specific errors.
#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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
    EALL,
}

impl Display for Code {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", &self)
    }
}

/// for converting from a text representation of the code to the code itself,
/// useful for `explain` command input to display error explanations.
impl std::convert::TryFrom<&str> for Code {
    type Error = anyhow::Error;
    fn try_from(code: &str) -> Result<Self, Self::Error> {
        match code {
            "E001" => Ok(Code::E001),
            "E002" => Ok(Code::E002),
            "E003" => Ok(Code::E003),
            "E004" => Ok(Code::E004),
            "E005" => Ok(Code::E005),
            "E006" => Ok(Code::E006),
            "E007" => Ok(Code::E007),
            "E008" => Ok(Code::E008),
            "E009" => Ok(Code::E009),
            "E010" => Ok(Code::E010),
            "E011" => Ok(Code::E011),
            "E012" => Ok(Code::E012),
            "E013" => Ok(Code::E013),
            "E014" => Ok(Code::E014),
            "E015" => Ok(Code::E015),
            "E016" => Ok(Code::E016),
            "E017" => Ok(Code::E017),
            "E018" => Ok(Code::E018),
            "E019" => Ok(Code::E019),
            "E020" => Ok(Code::E020),
            "E021" => Ok(Code::E021),
            "E022" => Ok(Code::E022),
            "E023" => Ok(Code::E023),
            "E024" => Ok(Code::E024),
            "E025" => Ok(Code::E025),
            "E026" => Ok(Code::E026),
            "E027" => Ok(Code::E027),
            "E028" => Ok(Code::E028),
            _ => Err(anyhow!("Invalid error code. Error codes are in the format `E###`"))
        }
    }
}

/// for converting from a text representation of the code to the code itself,
/// useful for `explain` command input to display error explanations.
impl FromStr for Code {
    type Err = anyhow::Error;
    fn from_str(code: &str) -> Result<Self, Self::Err> {
        match code {
            "E001" => Ok(Code::E001),
            "E002" => Ok(Code::E002),
            "E003" => Ok(Code::E003),
            "E004" => Ok(Code::E004),
            "E005" => Ok(Code::E005),
            "E006" => Ok(Code::E006),
            "E007" => Ok(Code::E007),
            "E008" => Ok(Code::E008),
            "E009" => Ok(Code::E009),
            "E010" => Ok(Code::E010),
            "E011" => Ok(Code::E011),
            "E012" => Ok(Code::E012),
            "E013" => Ok(Code::E013),
            "E014" => Ok(Code::E014),
            "E015" => Ok(Code::E015),
            "E016" => Ok(Code::E016),
            "E017" => Ok(Code::E017),
            "E018" => Ok(Code::E018),
            "E019" => Ok(Code::E019),
            "E020" => Ok(Code::E020),
            "E021" => Ok(Code::E021),
            "E022" => Ok(Code::E022),
            "E023" => Ok(Code::E023),
            "E024" => Ok(Code::E024),
            "E025" => Ok(Code::E025),
            "E026" => Ok(Code::E026),
            "E027" => Ok(Code::E027),
            "E028" => Ok(Code::E028),
            "EALL" => Ok(Code::EALL),
            _ => Err(anyhow!("Invalid error code. Error codes are in the format `E###`"))
        }
    }   
}


impl Code {
    // builds a BTreeMap of every possible code and its explanation, so we can
    // access from the `explain` function and get a single one OR so we can
    // iterate over them in `explain_all` for creating a docs page
    fn explanations() -> BTreeMap<Code, String>{
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
        ];
        BTreeMap::from_iter(contents.into_iter())
    }

    /// For a given error code, returns a markdown string with a given error's
    /// explanation. Explanations are in ./codes
    pub fn explain(&self) -> String {
        let all_explanations = Code::explanations();
        
        match self {
            // return all error explanations, concated with headings, for docs
            Code::EALL => {
                let mut all_md: String = "".to_string();

                for (code, expl) in all_explanations {
                    let pretty = format!("## {}\n\n{}\n\n", code, expl);
                    all_md.push_str(&pretty);
                };
    
                all_md
            },
            _ => {
                let explanation = all_explanations.get(self);
                if let Some(expl) = explanation {
                    // let heading = Red.underline().paint(self.to_string());
                    // add heading to md explanation
                    format!("**{}**\n\n{}\n\n", self.to_string(), expl.clone())
                } else {
                    "Explanation not available".to_string()
                }
            }
        }


    }
}
