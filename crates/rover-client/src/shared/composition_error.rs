use std::{
    error::Error,
    fmt::{self, Display},
};

use serde::Serialize;

#[derive(Debug, Serialize, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct CompositionErrors {
    pub errors: Vec<CompositionError>,
}

impl CompositionErrors {
    pub fn get_num_errors(&self) -> String {
        let num_failures = self.errors.len();
        if num_failures == 0 {
            unreachable!("No composition errors were encountered while composing the supergraph.");
        }

        match num_failures {
            1 => "1 composition error".to_string(),
            _ => format!("{} composition errors", num_failures),
        }
    }
}

impl Display for CompositionErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for composition_error in &self.errors {
            writeln!(f, "{}", composition_error)?;
        }
        Ok(())
    }
}

impl Error for CompositionError {}
impl Error for CompositionErrors {}
