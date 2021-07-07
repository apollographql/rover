use std::fmt::{self, Display};

#[derive(Debug, Clone, PartialEq)]
pub struct CompositionError {
    pub message: String,
    pub code: Option<String>,
}

impl Display for CompositionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(code) = &self.code {
            write!(f, "{}: ", code)?;
        } else {
            write!(f, "UNKNOWN: ")?;
        }
        write!(f, "{}", &self.message)
    }
}
