use crate::FieldType;
use std::fmt::{self, Display};

/// Field in a given SDL type.
#[derive(Debug, PartialEq, Clone)]
pub struct Field {
    description: Option<String>,
    //TODO(@lrlna): fields for objects types and interfaces can also take
    //arguments. This struct should also account for that.
    name: String,
    type_: FieldType,
    deprecated: bool,
    deprecation_reason: Option<String>,
}

impl Field {
    /// Create a new instance of Field.
    pub fn new(name: String, type_: FieldType) -> Self {
        Self {
            description: None,
            name,
            type_,
            deprecated: false,
            deprecation_reason: None,
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
}

impl Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(description) = &self.description {
            // Let's indent description on a field level for now, as all fields
            // are always on the same level and are indented by 2 spaces.
            writeln!(f, "  \"\"\"\n  {}\n  \"\"\"", description)?;
        }

        let mut deprecated = String::new();
        if self.deprecated {
            deprecated += " @deprecated";
            // Just in case deprecated field is ever used without a reason,
            // let's properly unwrap this Option.
            if let Some(reason) = &self.deprecation_reason {
                deprecated += &format!("(reason: \"{}\")", reason);
            }
        }
        write!(f, "  {}: {}{}", self.name, self.type_, deprecated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    #[test]
    fn it_encodes_fields() {
        let field_type_1 = FieldType::Type {
            ty: "SpaceProgram".to_string(),
            is_nullable: true,
            default: None,
        };

        let field_type_2 = FieldType::List {
            ty: Box::new(field_type_1),
            is_nullable: true,
        };

        let mut field = Field::new("cat".to_string(), field_type_2);
        field.description(Some("Very good cats".to_string()));
        field.deprecated(Some("Cats are no longer sent to space.".to_string()));

        assert_eq!(
            field.to_string(),
            r#"  """
  Very good cats
  """
  cat: [SpaceProgram] @deprecated(reason: "Cats are no longer sent to space.")"#
        );

        let field_type_4 = FieldType::Type {
            ty: "SpaceProgram".to_string(),
            is_nullable: false,
            default: None,
        };

        let field_type_5 = FieldType::List {
            ty: Box::new(field_type_4),
            is_nullable: false,
        };

        let mut field_2 = Field::new("spaceCat".to_string(), field_type_5);
        field_2.description(Some("Very good space cats".to_string()));

        assert_eq!(
            field_2.to_string(),
            r#"  """
  Very good space cats
  """
  spaceCat: [SpaceProgram!]!"#
        );

        let field_type_6 = FieldType::Type {
            ty: "SpaceProgram".to_string(),
            is_nullable: true,
            default: None,
        };

        let field_type_7 = FieldType::List {
            ty: Box::new(field_type_6),
            is_nullable: false,
        };

        let field_type_8 = FieldType::List {
            ty: Box::new(field_type_7),
            is_nullable: false,
        };

        let field_3 = Field::new("spaceCat".to_string(), field_type_8);

        assert_eq!(field_3.to_string(), r#"  spaceCat: [[SpaceProgram]!]!"#);
    }
}
