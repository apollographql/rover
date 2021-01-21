use std::fmt::{self, Display};

use ansi_term::Colour::{Cyan, Yellow};

#[derive(Debug)]
pub enum Suggestion {
    SubmitIssue,
    RerunWithSensitive,
}

impl Display for Suggestion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suggestion = match self {
            Suggestion::SubmitIssue => {
                format!("This error was unexpected! Please submit an issue with any relevant details about what you were trying to do: {}", Cyan.normal().paint("https://github.com/apollographql/rover/issues/new"))
            }
            Suggestion::RerunWithSensitive => {
                format!(
                    "Try re-running this command with the {} flag",
                    Yellow.normal().paint("'--sensitive'")
                )
            }
        };
        write!(formatter, "{}", &suggestion)
    }
}
