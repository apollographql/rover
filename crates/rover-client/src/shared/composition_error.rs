use std::{
    error::Error,
    fmt::{self, Display},
    iter::FromIterator,
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

#[derive(Debug, Default, Serialize, Clone, PartialEq)]
pub struct CompositionErrors {
    composition_errors: Vec<CompositionError>,
}

impl CompositionErrors {
    pub fn new() -> Self {
        CompositionErrors {
            composition_errors: Vec::new(),
        }
    }

    pub fn len(&self) -> String {
        let num_failures = self.composition_errors.len();
        if num_failures == 0 {
            unreachable!("No composition errors were encountered while composing the supergraph.");
        }

        match num_failures {
            1 => "1 composition error".to_string(),
            _ => format!("{} composition errors", num_failures),
        }
    }

    pub fn push(&mut self, error: CompositionError) {
        self.composition_errors.push(error);
    }

    pub fn is_empty(&self) -> bool {
        self.composition_errors.is_empty()
    }
}

impl Iterator for CompositionErrors {
    type Item = CompositionError;

    fn next(&mut self) -> Option<Self::Item> {
        self.composition_errors.clone().into_iter().next()
    }
}

impl Display for CompositionErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for composition_error in &self.composition_errors {
            writeln!(f, "{}", composition_error)?;
        }
        Ok(())
    }
}

impl From<Vec<CompositionError>> for CompositionErrors {
    fn from(composition_errors: Vec<CompositionError>) -> Self {
        CompositionErrors { composition_errors }
    }
}

impl FromIterator<CompositionError> for CompositionErrors {
    fn from_iter<I: IntoIterator<Item = CompositionError>>(iter: I) -> Self {
        let mut c = CompositionErrors::new();

        for i in iter {
            c.push(i);
        }

        c
    }
}

impl Error for CompositionError {}
impl Error for CompositionErrors {}
