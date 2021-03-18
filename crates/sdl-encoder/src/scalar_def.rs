use std::fmt::{self, Display};

/// Scalar type for SDL.
#[derive(Debug, PartialEq, Clone)]
pub struct ScalarDef {
    name: String,
    description: Option<String>,
}

impl ScalarDef {
    /// Create a new instance of Scalar type.
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
        }
    }

    /// Set the scalar def's description.
    pub fn description(&mut self, description: Option<String>) {
        self.description = description;
    }
}

impl Display for ScalarDef {
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

        writeln!(f, "scalar {}", self.name)
    }
}
