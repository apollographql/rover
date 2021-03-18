use crate::FieldValue;
use std::fmt::{self, Display};

/// Input Value struct.
#[derive(Debug, PartialEq, Clone)]
pub struct InputValue {
    description: Option<String>,
    name: String,
    type_: FieldValue,
    deprecated: bool,
    deprecation_reason: Option<String>,
    default: Option<String>,
}

impl InputValue {
    /// Create a new instance of InputValue.
    pub fn new(name: String, type_: FieldValue) -> Self {
        Self {
            description: None,
            name,
            type_,
            deprecated: false,
            deprecation_reason: None,
            default: None,
        }
    }

    /// Set the Input Value's description.
    pub fn description(&mut self, description: Option<String>) {
        self.description = description;
    }

    /// Set the Input Value's default value.
    pub fn default(&mut self, default: Option<String>) {
        self.default = default;
    }

    /// Set the Input Value's deprecation properties.
    pub fn deprecated(&mut self, reason: Option<String>) {
        self.deprecated = true;
        self.deprecation_reason = reason;
    }
}

impl Display for InputValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(description) = &self.description {
            // We are determing on whether to have description formatted as
            // a multiline comment based on whether or not it already includes a
            // \n.
            match description.contains('\n') {
                true => write!(f, "\"\"\"\n{}\n\"\"\" ", description)?,
                false => write!(f, "\"\"\"{}\"\"\" ", description)?,
            }
        }

        write!(f, "{}: {}", self.name, self.type_)?;

        if let Some(default) = &self.default {
            write!(f, " = {}", default)?;
        }

        if self.deprecated {
            write!(f, " @deprecated")?;
            // Just in case deprecated field is ever used without a reason,
            // let's properly unwrap this Option.
            if let Some(reason) = &self.deprecation_reason {
                write!(f, "(reason: \"{}\")", reason)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn it_encodes_simple_values() {
        let ty_1 = FieldValue::Type {
            ty: "SpaceProgram".to_string(),
            default: None,
        };

        let ty_2 = FieldValue::List { ty: Box::new(ty_1) };
        let ty_3 = FieldValue::NonNull { ty: Box::new(ty_2) };
        let value = InputValue::new("spaceCat".to_string(), ty_3);

        assert_eq!(value.to_string(), r#"spaceCat: [SpaceProgram]!"#);
    }

    #[test]
    fn it_encodes_input_values_with_default() {
        let ty_1 = FieldValue::Type {
            ty: "Breed".to_string(),
            default: None,
        };

        let ty_2 = FieldValue::NonNull { ty: Box::new(ty_1) };
        let mut value = InputValue::new("spaceCat".to_string(), ty_2);
        value.default(Some("\"Norwegian Forest\"".to_string()));

        assert_eq!(
            value.to_string(),
            r#"spaceCat: Breed! = "Norwegian Forest""#
        );
    }

    #[test]
    fn it_encodes_valueument_with_deprecation() {
        let ty_1 = FieldValue::Type {
            ty: "SpaceProgram".to_string(),
            default: None,
        };

        let ty_2 = FieldValue::List { ty: Box::new(ty_1) };
        let mut value = InputValue::new("cat".to_string(), ty_2);
        value.description(Some("Very good cats".to_string()));
        value.deprecated(Some("Cats are no longer sent to space.".to_string()));

        assert_eq!(
            value.to_string(),
            r#""""Very good cats""" cat: [SpaceProgram] @deprecated(reason: "Cats are no longer sent to space.")"#
        );
    }

    #[test]
    fn it_encodes_valueuments_with_description() {
        let ty_1 = FieldValue::Type {
            ty: "SpaceProgram".to_string(),
            default: None,
        };

        let ty_2 = FieldValue::NonNull { ty: Box::new(ty_1) };
        let ty_3 = FieldValue::List { ty: Box::new(ty_2) };
        let ty_4 = FieldValue::NonNull { ty: Box::new(ty_3) };
        let mut value = InputValue::new("spaceCat".to_string(), ty_4);
        value.description(Some("Very good space cats".to_string()));

        assert_eq!(
            value.to_string(),
            r#""""Very good space cats""" spaceCat: [SpaceProgram!]!"#
        );
    }
}
