use std::fmt::{self, Display};

/// `Code` contains the error codes associated with specific errors.
#[derive(Debug)]
pub enum Code {}

impl Display for Code {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", &self)
    }
}
