use std::{
    error::Error,
    fmt::{self, Display},
    iter::FromIterator,
};

use serde::{ser::SerializeSeq, Deserialize, Serialize, Serializer};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
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

#[derive(Debug, Deserialize, Default, Clone, PartialEq)]
pub struct CompositionErrors {
    composition_errors: Vec<CompositionError>,
}

impl Serialize for CompositionErrors {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.composition_errors.len()))?;
        for composition_error in &self.composition_errors {
            sequence.serialize_element(composition_error)?;
        }
        sequence.end()
    }
}

impl CompositionErrors {
    pub fn new() -> Self {
        CompositionErrors {
            composition_errors: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.composition_errors.len()
    }

    pub fn length_string(&self) -> String {
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

#[cfg(test)]
mod tests {
    use super::{CompositionError, CompositionErrors};

    use serde_json::{json, Value};

    #[test]
    fn it_can_serialize_empty_errors() {
        let composition_errors = CompositionErrors::new();
        assert_eq!(
            serde_json::to_string(&composition_errors)
                .expect("Could not serialize composition errors"),
            json!([]).to_string()
        );
    }

    #[test]
    fn it_can_serialize_some_composition_errors() {
        let composition_errors: CompositionErrors = vec![
            CompositionError {
                code: None,
                message: "wow".to_string(),
            },
            CompositionError {
                code: Some("BOO".to_string()),
                message: "boo".to_string(),
            },
        ]
        .into();

        let actual_value: Value = serde_json::from_str(
            &serde_json::to_string(&composition_errors)
                .expect("Could not convert composition errors to string"),
        )
        .expect("Could not convert composition error string to serde_json::Value");

        let expected_value = json!([
          {
            "code": null,
            "message": "wow",
          },
          {
            "code": "BOO",
            "message": "boo",
          }
        ]);
        assert_eq!(actual_value, expected_value);
    }
}
