use crate::Field;
use std::fmt::{self, Display};

/// Input types used to describe more complex objects in SDL.
#[derive(Debug, Clone)]
pub struct InputDef {
    name: String,
    description: Option<String>,
    interfaces: Vec<String>,
    fields: Vec<Field>,
}

impl InputDef {
    /// Create a new instance of ObjectDef with a name.
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            fields: Vec::new(),
            interfaces: Vec::new(),
        }
    }

    /// Set the interfaces InputDef implements.
    pub fn interface(&mut self, interface: String) {
        self.interfaces.push(interface)
    }

    /// Set the ObjectDef's description field.
    pub fn description(&mut self, description: Option<String>) {
        self.description = description
    }

    /// Push a Field to type def's fields vector.
    pub fn field(&mut self, field: Field) {
        self.fields.push(field)
    }
}

impl Display for InputDef {
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

        write!(f, "input {}{} {{", &self.name, interfaces)?;
        write!(f, "{}", fields)?;
        writeln!(f, "\n}}")
    }
}
