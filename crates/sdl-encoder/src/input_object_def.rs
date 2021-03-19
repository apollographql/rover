use crate::Field;
use std::fmt::{self, Display};

/// Input objects are composite types used as inputs into queries defined as a list of named input values..
///
/// InputObjectTypeDefinition
///     Description<sub>opt</sub> **input** Name Directives<sub>\[Const\] opt</sub> FieldsDefinition<sub>opt</sub>
///
/// Detailed documentation can be found in [GraphQL spec](https://spec.graphql.org/draft/#sec-Input-Objects).
///
/// **Note**: At the moment InputObjectTypeDefinition differs slightly from the
/// spec. Instead of accepting InputValues as `field` parameter, we accept
/// Fields.
///
/// ### Example
/// ```rust
/// use sdl_encoder::{FieldValue, Field, InputObjectDef};
/// use indoc::indoc;
///
/// let ty_1 = FieldValue::Type {
///     ty: "DanglerPoleToys".to_string(),
/// };
///
/// let ty_2 = FieldValue::List { ty: Box::new(ty_1) };
/// let mut field = Field::new("toys".to_string(), ty_2);
/// field.default(Some("\"Cat Dangler Pole Bird\"".to_string()));
/// let ty_3 = FieldValue::Type {
///     ty: "FavouriteSpots".to_string(),
/// };
/// let mut field_2 = Field::new("playSpot".to_string(), ty_3);
/// field_2.description(Some("Best playime spots, e.g. tree, bed.".to_string()));
///
/// let mut input_def = InputObjectDef::new("PlayTime".to_string());
/// input_def.field(field);
/// input_def.field(field_2);
/// input_def.description(Some("Cat playtime input".to_string()));
///
/// assert_eq!(
///     input_def.to_string(),
///     indoc! { r#"
///         """Cat playtime input"""
///         input PlayTime {
///           toys: [DanglerPoleToys] = "Cat Dangler Pole Bird"
///           """Best playime spots, e.g. tree, bed."""
///           playSpot: FavouriteSpots
///         }
///     "#}
/// );
/// ```
#[derive(Debug, Clone)]
pub struct InputObjectDef {
    // Name must return a String.
    name: String,
    // Description may return a String or null.
    description: Option<String>,
    // The vector of interfaces that this object implements.
    interfaces: Vec<String>,
    // A vector of fields
    fields: Vec<Field>,
}

impl InputObjectDef {
    /// Create a new instance of ObjectDef with a name.
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            fields: Vec::new(),
            interfaces: Vec::new(),
        }
    }

    /// Set the interfaces InputObjectDef implements.
    pub fn interface(&mut self, interface: String) {
        self.interfaces.push(interface)
    }

    /// Set the InputObjectDef's description field.
    pub fn description(&mut self, description: Option<String>) {
        self.description = description
    }

    /// Push a Field to InputObjectDef's fields vector.
    pub fn field(&mut self, field: Field) {
        self.fields.push(field)
    }
}

impl Display for InputObjectDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(description) = &self.description {
            // We are determing on whether to have description formatted as
            // a multiline comment based on whether or not it already includes a
            // \n.
            match description.contains('\n') {
                true => writeln!(f, "\"\"\"\n{}\n\"\"\"", description)?,
                false => writeln!(f, "\"\"\"{}\"\"\"", description)?,
            }
        }

        write!(f, "input {}", &self.name)?;
        for (i, interface) in self.interfaces.iter().enumerate() {
            match i {
                0 => write!(f, " implements {}", interface)?,
                _ => write!(f, "& {}", interface)?,
            }
        }
        write!(f, " {{")?;

        for field in &self.fields {
            write!(f, "\n{}", field)?;
        }
        writeln!(f, "\n}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Field, FieldValue};
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn it_encodes_input_object() {
        let ty_1 = FieldValue::Type {
            ty: "DanglerPoleToys".to_string(),
        };

        let ty_2 = FieldValue::List { ty: Box::new(ty_1) };
        let mut field = Field::new("toys".to_string(), ty_2);
        field.default(Some("\"Cat Dangler Pole Bird\"".to_string()));
        let ty_3 = FieldValue::Type {
            ty: "FavouriteSpots".to_string(),
        };
        let mut field_2 = Field::new("playSpot".to_string(), ty_3);
        field_2.description(Some("Best playime spots, e.g. tree, bed.".to_string()));

        let mut input_def = InputObjectDef::new("PlayTime".to_string());
        input_def.field(field);
        input_def.field(field_2);

        assert_eq!(
            input_def.to_string(),
            indoc! { r#"
                input PlayTime {
                  toys: [DanglerPoleToys] = "Cat Dangler Pole Bird"
                  """Best playime spots, e.g. tree, bed."""
                  playSpot: FavouriteSpots
                }
            "#}
        );
    }

    #[test]
    fn it_encodes_input_object_with_description() {
        let ty_1 = FieldValue::Type {
            ty: "DanglerPoleToys".to_string(),
        };

        let ty_2 = FieldValue::List { ty: Box::new(ty_1) };
        let mut field = Field::new("toys".to_string(), ty_2);
        field.default(Some("\"Cat Dangler Pole Bird\"".to_string()));
        let ty_3 = FieldValue::Type {
            ty: "FavouriteSpots".to_string(),
        };
        let mut field_2 = Field::new("playSpot".to_string(), ty_3);
        field_2.description(Some("Best playime spots, e.g. tree, bed.".to_string()));

        let mut input_def = InputObjectDef::new("PlayTime".to_string());
        input_def.field(field);
        input_def.field(field_2);
        input_def.description(Some("Cat playtime input".to_string()));

        assert_eq!(
            input_def.to_string(),
            indoc! { r#"
                """Cat playtime input"""
                input PlayTime {
                  toys: [DanglerPoleToys] = "Cat Dangler Pole Bird"
                  """Best playime spots, e.g. tree, bed."""
                  playSpot: FavouriteSpots
                }
            "#}
        );
    }

    #[test]
    fn it_encodes_input_object_with_interfaces() {
        let ty_1 = FieldValue::Type {
            ty: "DanglerPoleToys".to_string(),
        };

        let ty_2 = FieldValue::List { ty: Box::new(ty_1) };
        let mut field = Field::new("toys".to_string(), ty_2);
        field.default(Some("\"Cat Dangler Pole Bird\"".to_string()));
        let ty_3 = FieldValue::Type {
            ty: "FavouriteSpots".to_string(),
        };
        let mut field_2 = Field::new("playSpot".to_string(), ty_3);
        field_2.description(Some("Best playime spots, e.g. tree, bed.".to_string()));

        let mut input_def = InputObjectDef::new("PlayTime".to_string());
        input_def.field(field);
        input_def.field(field_2);
        input_def.interface("NonNapTimeActivity".to_string());

        assert_eq!(
            input_def.to_string(),
            indoc! { r#"
                input PlayTime implements NonNapTimeActivity {
                  toys: [DanglerPoleToys] = "Cat Dangler Pole Bird"
                  """Best playime spots, e.g. tree, bed."""
                  playSpot: FavouriteSpots
                }
            "#}
        );
    }
}
