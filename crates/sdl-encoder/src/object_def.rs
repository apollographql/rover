use crate::Field;
use std::fmt::{self, Display};
/// Object Type Definition used to define different objects in SDL.
#[derive(Debug)]
pub struct ObjectDef {
    name: String,
    description: Option<String>,
    interfaces: Vec<String>,
    fields: Vec<Field>,
}

impl ObjectDef {
    /// Create a new instance of ObjectDef with a name.
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            interfaces: Vec::new(),
            fields: Vec::new(),
        }
    }

    /// Set the ObjectDef's description field.
    pub fn description(&mut self, description: Option<String>) {
        self.description = description
    }

    /// Set the interfaces ObjectDef implements.
    pub fn interface(&mut self, interface: String) {
        self.interfaces.push(interface)
    }

    /// Push a Field to type def's fields vector.
    pub fn field(&mut self, field: Field) {
        self.fields.push(field)
    }
}

impl Display for ObjectDef {
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

        write!(f, "type {}{} {{", &self.name, interfaces)?;
        write!(f, "{}", fields)?;
        writeln!(f, "\n}}")
    }
}
