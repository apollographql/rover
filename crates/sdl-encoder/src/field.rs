use crate::{FieldValue, InputValue};
use std::fmt::{self, Display};
/// Field in a given SDL type.
#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    description: Option<String>,
    name: String,
    type_: FieldValue,
    values: Vec<InputValue>,
    deprecated: bool,
    deprecation_reason: Option<String>,
    default: Option<String>,
}

impl Field {
    /// Create a new instance of Field.
    pub fn new(name: String, type_: FieldValue) -> Self {
        Self {
            description: None,
            name,
            type_,
            values: Vec::new(),
            deprecated: false,
            deprecation_reason: None,
            default: None,
        }
    }

    /// Set the field's description.
    pub fn description(&mut self, description: Option<String>) {
        self.description = description;
    }

    /// Set the field's deprecation properties.
    pub fn deprecated(&mut self, reason: Option<String>) {
        self.deprecated = true;
        self.deprecation_reason = reason;
    }

    /// Set the Field's default value.
    pub fn default(&mut self, default: Option<String>) {
        self.default = default;
    }

    /// Set the field's values.
    pub fn value(&mut self, value: InputValue) {
        self.values.push(value);
    }
}

impl Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(description) = &self.description {
            // Let's indent description on a field level for now, as all fields
            // are always on the same level and are indented by 2 spaces.
            //
            // We are also determing on whether to have description formatted as
            // a multiline comment based on whether or not it already includes a
            // \n.
            match description.contains('\n') {
                true => writeln!(f, "  \"\"\"\n  {}\n  \"\"\"", description)?,
                false => writeln!(f, "  \"\"\"{}\"\"\"", description)?,
            }
        }

        write!(f, "  {}", self.name)?;

        if !self.values.is_empty() {
            for (i, value) in self.values.iter().enumerate() {
                match i {
                    0 => write!(f, "({}", value)?,
                    _ => write!(f, ", {}", value)?,
                }
            }
            write!(f, ")")?;
        }

        write!(f, ": {}", self.type_)?;

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
    fn it_encodes_simple_fields() {
        let ty_1 = FieldValue::Type {
            ty: "SpaceProgram".to_string(),
            default: None,
        };

        let ty_2 = FieldValue::List { ty: Box::new(ty_1) };
        let ty_3 = FieldValue::NonNull { ty: Box::new(ty_2) };
        let field = Field::new("spaceCat".to_string(), ty_3);

        assert_eq!(field.to_string(), r#"  spaceCat: [SpaceProgram]!"#);
    }

    #[test]
    fn it_encodes_fields_with_deprecation() {
        let ty_1 = FieldValue::Type {
            ty: "SpaceProgram".to_string(),
            default: None,
        };

        let ty_2 = FieldValue::List { ty: Box::new(ty_1) };
        let mut field = Field::new("cat".to_string(), ty_2);
        field.description(Some("Very good cats".to_string()));
        field.deprecated(Some("Cats are no longer sent to space.".to_string()));

        assert_eq!(
            field.to_string(),
            r#"  """Very good cats"""
  cat: [SpaceProgram] @deprecated(reason: "Cats are no longer sent to space.")"#
        );
    }

    #[test]
    fn it_encodes_fields_with_description() {
        let ty_1 = FieldValue::Type {
            ty: "SpaceProgram".to_string(),
            default: None,
        };

        let ty_2 = FieldValue::NonNull { ty: Box::new(ty_1) };
        let ty_3 = FieldValue::List { ty: Box::new(ty_2) };
        let ty_4 = FieldValue::NonNull { ty: Box::new(ty_3) };
        let mut field = Field::new("spaceCat".to_string(), ty_4);
        field.description(Some("Very good space cats".to_string()));

        assert_eq!(
            field.to_string(),
            r#"  """Very good space cats"""
  spaceCat: [SpaceProgram!]!"#
        );
    }

    #[test]
    fn it_encodes_fields_with_valueuments() {
        let ty_1 = FieldValue::Type {
            ty: "SpaceProgram".to_string(),
            default: None,
        };

        let ty_2 = FieldValue::NonNull { ty: Box::new(ty_1) };
        let ty_3 = FieldValue::List { ty: Box::new(ty_2) };
        let ty_4 = FieldValue::NonNull { ty: Box::new(ty_3) };
        let mut field = Field::new("spaceCat".to_string(), ty_4);
        field.description(Some("Very good space cats".to_string()));

        let value_1 = FieldValue::Type {
            ty: "SpaceProgram".to_string(),
            default: None,
        };

        let value_2 = FieldValue::List {
            ty: Box::new(value_1),
        };
        let mut value = InputValue::new("cat".to_string(), value_2);
        value.deprecated(Some("Cats are no longer sent to space.".to_string()));
        field.value(value);

        assert_eq!(
            field.to_string(),
            r#"  """Very good space cats"""
  spaceCat(cat: [SpaceProgram] @deprecated(reason: "Cats are no longer sent to space.")): [SpaceProgram!]!"#
        );
    }

    #[test]
    fn it_encodes_fields_with_defaults() {
        let ty_1 = FieldValue::Type {
            ty: "CatBreed".to_string(),
            default: None,
        };

        let mut field = Field::new("cat".to_string(), ty_1);

        let value_1 = FieldValue::Type {
            ty: "CatBreed".to_string(),
            default: None,
        };

        let value = InputValue::new("breed".to_string(), value_1);

        field.value(value);
        field.default(Some("\"Norwegian Forest\"".to_string()));

        assert_eq!(
            field.to_string(),
            r#"  cat(breed: CatBreed): CatBreed = "Norwegian Forest""#
        );
    }
}
