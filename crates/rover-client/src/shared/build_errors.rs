use std::{
    error::Error,
    fmt::{self, Display},
    iter::FromIterator,
};

use serde::{ser::SerializeSeq, Deserialize, Serialize, Serializer};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct BuildError {
    message: String,
    code: Option<String>,
    r#type: BuildErrorType,
}

impl BuildError {
    pub fn composition_error(message: String, code: Option<String>) -> BuildError {
        BuildError {
            message,
            code,
            r#type: BuildErrorType::Composition,
        }
    }

    pub fn get_message(&self) -> String {
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
        if let Some(code) = &self.code {
            write!(f, "{}: ", code)?;
        } else {
            write!(f, "UNKNOWN: ")?;
        }
        write!(f, "{}", &self.message)
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
        for build_error in &self.build_errors {
            writeln!(f, "{}", build_error)?;
        }
        Ok(())
    }
}

impl From<Vec<BuildError>> for BuildErrors {
    fn from(build_errors: Vec<BuildError>) -> Self {
        BuildErrors {
            build_errors: build_errors,
        }
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
            BuildError::composition_error("wow".to_string(), None),
            BuildError::composition_error("boo".to_string(), Some("BOO".to_string())),
        ]
        .into();

        let actual_value: Value = serde_json::from_str(
            &serde_json::to_string(&build_errors)
                .expect("Could not convert build errors to string"),
        )
        .expect("Could not convert build error string to serde_json::Value");

        let expected_value = json!([
          {
            "code": null,
            "message": "wow",
            "type": "composition"
          },
          {
            "code": "BOO",
            "message": "boo",
            "type": "composition"
          }
        ]);
        assert_eq!(actual_value, expected_value);
    }
}
