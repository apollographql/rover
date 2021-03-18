use std::fmt::{self, Display};

/// Enum Value type.
#[derive(Debug, PartialEq, Clone)]
pub struct EnumValue {
    name: String,
    deprecated: bool,
    description: Option<String>,
    deprecation_reason: Option<String>,
}

impl EnumValue {
    /// Create a new instance of EnumValue.
    pub fn new(name: String) -> Self {
        Self {
            name,
            deprecated: false,
            description: None,
            deprecation_reason: None,
        }
    }

    /// Set the Enum Value's description.
    pub fn description(&mut self, description: Option<String>) {
        self.description = description;
    }

    /// Set the Enum Value's deprecation properties.
    pub fn deprecated(&mut self, reason: Option<String>) {
        self.deprecated = true;
        self.deprecation_reason = reason;
    }
}

impl Display for EnumValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(description) = &self.description {
            writeln!(f, "  \"\"\"\n  {}\n  \"\"\"", description)?;
        }

        write!(f, "  {}", self.name)?;

        if self.deprecated {
            write!(f, " @deprecated")?;
            // Just in case deprecated directive is ever used without a reason,
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
    fn it_encodes_an_enum_value() {
        let enum_ty = EnumValue::new("CatTree".to_string());
        assert_eq!(enum_ty.to_string(), "  CatTree");
    }

    #[test]
    fn it_encodes_an_enum_value_with_desciption() {
        let mut enum_ty = EnumValue::new("CatTree".to_string());
        enum_ty.description(Some("Top bunk of a cat tree.".to_string()));
        assert_eq!(
            enum_ty.to_string(),
            r#"  """
  Top bunk of a cat tree.
  """
  CatTree"#
        );
    }
    #[test]
    fn it_encodes_an_enum_value_with_deprecated() {
        let mut enum_ty = EnumValue::new("CardboardBox".to_string());
        enum_ty.description(Some("Box nap spot.".to_string()));
        enum_ty.deprecated(Some("Box was recycled.".to_string()));

        assert_eq!(
            enum_ty.to_string(),
            r#"  """
  Box nap spot.
  """
  CardboardBox @deprecated(reason: "Box was recycled.")"#
        );
    }
}
