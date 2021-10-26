use std::{
    error::Error,
    fmt::{self, Display},
};

use serde::{ser::SerializeSeq, Deserialize, Serialize, Serializer};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct BuildError {
    message: Option<String>,
    code: Option<String>,
    r#type: BuildErrorType,
}

impl BuildError {
    pub fn composition_error(code: Option<String>, message: Option<String>) -> BuildError {
        BuildError {
            code,
            message,
            r#type: BuildErrorType::Composition,
        }
    }

    pub fn get_message(&self) -> Option<String> {
        self.message.clone()
    }

    pub fn get_code(&self) -> Option<String> {
        self.code.clone()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BuildErrorType {
    Composition,
}

impl Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.code.as_ref().map_or("UNKNOWN", String::as_str)
        )?;
        if let Some(message) = &self.message {
            write!(f, ": {}", message)?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Default, Clone, PartialEq)]
pub struct BuildErrors {
    build_errors: Vec<BuildError>,
}

impl Serialize for BuildErrors {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut sequence = serializer.serialize_seq(Some(self.build_errors.len()))?;
        for build_error in &self.build_errors {
            sequence.serialize_element(build_error)?;
        }
        sequence.end()
    }
}

impl BuildErrors {
    pub fn new() -> Self {
        BuildErrors {
            build_errors: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.build_errors.len()
    }

    pub fn length_string(&self) -> String {
        let num_failures = self.build_errors.len();
        if num_failures == 0 {
            unreachable!("No build errors were encountered while composing the supergraph.");
        }

        match num_failures {
            1 => "1 build error".to_string(),
            _ => format!("{} build errors", num_failures),
        }
    }

    pub fn push(&mut self, error: BuildError) {
        self.build_errors.push(error);
    }

    pub fn is_empty(&self) -> bool {
        self.build_errors.is_empty()
    }
}

impl Display for BuildErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let num_failures = self.build_errors.len();
        if num_failures == 0
            || (num_failures == 1
                && self.build_errors[0].code.is_none()
                && self.build_errors[0].message.is_none())
        {
            writeln!(f, "Something went wrong! No build errors were recorded, but we also build a valid supergraph SDL.")?;
        } else {
            let length_message = if num_failures == 1 {
                "1 build error".to_string()
            } else {
                format!("{} build errors", num_failures)
            };
            writeln!(
                f,
                "Encountered {} while trying to build the supergraph.",
                &length_message
            )?;
            for build_error in &self.build_errors {
                writeln!(f, "{}", build_error)?;
            }
        }
        Ok(())
    }
}

impl From<Vec<BuildError>> for BuildErrors {
    fn from(build_errors: Vec<BuildError>) -> Self {
        BuildErrors { build_errors }
    }
}

impl FromIterator<BuildError> for BuildErrors {
    fn from_iter<I: IntoIterator<Item = BuildError>>(iter: I) -> Self {
        let mut c = BuildErrors::new();

        for i in iter {
            c.push(i);
        }

        c
    }
}

impl Error for BuildError {}
impl Error for BuildErrors {}

#[cfg(test)]
mod tests {
    use super::{BuildError, BuildErrors};

    use serde_json::{json, Value};

    #[test]
    fn it_can_serialize_empty_errors() {
        let build_errors = BuildErrors::new();
        assert_eq!(
            serde_json::to_string(&build_errors).expect("Could not serialize build errors"),
            json!([]).to_string()
        );
    }

    #[test]
    fn it_can_serialize_some_build_errors() {
        let build_errors: BuildErrors = vec![
            BuildError::composition_error(None, Some("wow".to_string())),
            BuildError::composition_error(Some("BOO".to_string()), Some("boo".to_string())),
        ]
        .into();

        let actual_value: Value = serde_json::from_str(
            &serde_json::to_string(&build_errors)
                .expect("Could not convert build errors to string"),
        )
        .expect("Could not convert build error string to serde_json::Value");

        let expected_value = json!([
          {
            "message": "wow",
            "code": null,
            "type": "composition"
          },
          {
            "message": "boo",
            "code": "BOO",
            "type": "composition"
          }
        ]);
        assert_eq!(actual_value, expected_value);
    }
}
