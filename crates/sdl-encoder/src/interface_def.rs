use crate::Field;
use std::fmt::{self, Display};

/// A definition used when a root GraphQL type differs from default types.
#[derive(Debug, Clone)]
pub struct Interface {
    name: String,
    description: Option<String>,
    interfaces: Vec<String>,
    fields: Vec<Field>,
}

impl Interface {
    /// Create a new instance of Interface.
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            fields: Vec::new(),
            interfaces: Vec::new(),
        }
    }

    /// Set the schema def's description.
    pub fn description(&mut self, description: Option<String>) {
        self.description = description
    }

    /// Set the interfaces ObjectDef implements.
    pub fn interface(&mut self, interface: String) {
        self.interfaces.push(interface)
    }

    /// Push a Field to schema def's fields vector.
    pub fn field(&mut self, field: Field) {
        self.fields.push(field)
    }
}

impl Display for Interface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(description) = &self.description {
            writeln!(f, "\"\"\"\n{}\n\"\"\"", description)?;
        }

        let mut fields = String::new();
        for field in &self.fields {
            fields += &format!("\n{}", field.to_string());
        }

        let mut interfaces = String::new();
        for (i, interface) in self.interfaces.iter().enumerate() {
            if i == 0 {
                interfaces += &format!(" implements {}", interface);
                continue;
            }
            interfaces += &format!(" & {}", interface);
        }

        write!(f, "interface {}{} {{", &self.name, interfaces)?;
        write!(f, "{}", fields)?;
        writeln!(f, "\n}}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FieldValue;
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn it_encodes_interfaces() {
        let ty_1 = FieldValue::Type {
            ty: "String".to_string(),
            default: None,
        };

        let ty_2 = FieldValue::Type {
            ty: "String".to_string(),
            default: None,
        };

        let ty_3 = FieldValue::NonNull { ty: Box::new(ty_2) };
        let ty_4 = FieldValue::List { ty: Box::new(ty_3) };
        let ty_5 = FieldValue::NonNull { ty: Box::new(ty_4) };

        let ty_6 = FieldValue::Type {
            ty: "Boolean".to_string(),
            default: None,
        };

        let mut field_1 = Field::new("main".to_string(), ty_1);
        field_1.description(Some("Cat's main dish of a meal.".to_string()));

        let mut field_2 = Field::new("snack".to_string(), ty_5);
        field_2.description(Some("Cat's post meal snack.".to_string()));

        let mut field_3 = Field::new("pats".to_string(), ty_6);
        field_3.description(Some("Does cat get a pat after meal?".to_string()));

        // a schema definition
        let mut interface = Interface::new("Meal".to_string());
        interface.description(Some(
            "Meal interface for various meals during the day.".to_string(),
        ));
        interface.field(field_1);
        interface.field(field_2);
        interface.field(field_3);

        assert_eq!(
            interface.to_string(),
            indoc! { r#"
            """
            Meal interface for various meals during the day.
            """
            interface Meal {
              """
              Cat's main dish of a meal.
              """
              main: String
              """
              Cat's post meal snack.
              """
              snack: [String!]!
              """
              Does cat get a pat after meal?
              """
              pats: Boolean
            }
            "# }
        );
    }
}
