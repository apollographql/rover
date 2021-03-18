use crate::EnumValue;
use std::fmt::{self, Display};

/// Enum type in SDL.
#[derive(Debug, PartialEq, Clone)]
pub struct EnumDef {
    name: String,
    description: Option<String>,
    values: Vec<EnumValue>,
}

impl EnumDef {
    /// Create a new Enum type for SDL.
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            values: Vec::new(),
        }
    }

    /// Set the enum's description.
    pub fn description(&mut self, description: Option<String>) {
        self.description = description;
    }

    /// Set the EnumDef's values.
    pub fn value(&mut self, value: EnumValue) {
        self.values.push(value)
    }
}

impl Display for EnumDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(description) = &self.description {
            writeln!(f, "\"\"\"\n{}\n\"\"\"", description)?;
        }

        write!(f, "enum {} {{", self.name)?;
        for value in &self.values {
            write!(f, "\n{}", value)?;
        }
        writeln!(f, "\n}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn it_encodes_a_simple_enum() {
        let enum_ty_1 = EnumValue::new("CatTree".to_string());
        let enum_ty_2 = EnumValue::new("Bed".to_string());
        let enum_ty_3 = EnumValue::new("CardboardBox".to_string());

        let mut enum_ = EnumDef::new("NapSpots".to_string());
        enum_.value(enum_ty_1);
        enum_.value(enum_ty_2);
        enum_.value(enum_ty_3);

        assert_eq!(
            enum_.to_string(),
            r#"enum NapSpots {
  CatTree
  Bed
  CardboardBox
}
"#
        );
    }
    #[test]
    fn it_encodes_enum_with_descriptions() {
        let mut enum_ty_1 = EnumValue::new("CatTree".to_string());
        enum_ty_1.description(Some("Top bunk of a cat tree.".to_string()));
        let enum_ty_2 = EnumValue::new("Bed".to_string());
        let mut enum_ty_3 = EnumValue::new("CardboardBox".to_string());
        enum_ty_3.deprecated(Some("Box was recycled.".to_string()));

        let mut enum_ = EnumDef::new("NapSpots".to_string());
        enum_.description(Some("Favourite cat nap spots.".to_string()));
        enum_.value(enum_ty_1);
        enum_.value(enum_ty_2);
        enum_.value(enum_ty_3);

        assert_eq!(
            enum_.to_string(),
            r#""""
Favourite cat nap spots.
"""
enum NapSpots {
  """
  Top bunk of a cat tree.
  """
  CatTree
  Bed
  CardboardBox @deprecated(reason: "Box was recycled.")
}
"#
        );
    }
}
