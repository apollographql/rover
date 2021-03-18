use crate::FieldArgument;
use std::fmt::{self, Display};

/// The __Directive type represents a Directive that a service supports.
#[derive(Debug)]
pub struct Directive {
    name: String,
    description: Option<String>,
    args: Vec<FieldArgument>,
    locations: Vec<String>,
}

impl Directive {
    /// Create a new instance of Directive type.
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            args: Vec::new(),
            locations: Vec::new(),
        }
    }

    /// Set the Directive's description.
    pub fn description(&mut self, description: Option<String>) {
        self.description = description;
    }

    /// Set the Directive's location.
    pub fn location(&mut self, location: String) {
        self.locations.push(location);
    }

    /// Set the Directive's args.
    pub fn arg(&mut self, arg: FieldArgument) {
        self.args.push(arg);
    }
}

impl Display for Directive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(description) = &self.description {
            writeln!(f, "\"\"\"\n{}\n\"\"\"", description)?;
        }

        write!(f, "directive @{}", self.name)?;

        if !self.args.is_empty() {
            for (i, arg) in self.args.iter().enumerate() {
                match i {
                    0 => write!(f, "({}", arg)?,
                    _ => write!(f, ", {}", arg)?,
                }
            }
            write!(f, ")")?;
        }

        for (i, location) in self.locations.iter().enumerate() {
            match i {
                0 => write!(f, " on {}", location)?,
                _ => write!(f, " | {}", location)?,
            }
        }

        // append a new line at the end
        writeln!(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // use indoc::indoc;
    use crate::FieldType;
    use pretty_assertions::assert_eq;

    #[test]
    fn it_encodes_directives_for_a_single_location() {
        let mut directive = Directive::new("infer".to_string());
        directive.description(Some("Infer field types from field values.".to_string()));
        directive.location("OBJECT".to_string());

        assert_eq!(
            directive.to_string(),
            r#""""
Infer field types from field values.
"""
directive @infer on OBJECT
"#
        );
    }

    #[test]
    fn it_encodes_directives_for_multiple_location() {
        let mut directive = Directive::new("infer".to_string());
        directive.description(Some("Infer field types from field values.".to_string()));
        directive.location("OBJECT".to_string());
        directive.location("FIELD_DEFINITION".to_string());
        directive.location("INPUT_FIELD_DEFINITION".to_string());

        assert_eq!(
            directive.to_string(),
            r#""""
Infer field types from field values.
"""
directive @infer on OBJECT | FIELD_DEFINITION | INPUT_FIELD_DEFINITION
"#
        );
    }

    #[test]
    fn it_encodes_directives_with_arguments() {
        let mut directive = Directive::new("infer".to_string());
        directive.description(Some("Infer field types from field values.".to_string()));
        directive.location("OBJECT".to_string());

        let ty_1 = FieldType::Type {
            ty: "SpaceProgram".to_string(),
            default: None,
        };

        let ty_2 = FieldType::List { ty: Box::new(ty_1) };
        let arg = FieldArgument::new("cat".to_string(), ty_2);
        directive.arg(arg);

        assert_eq!(
            directive.to_string(),
            r#""""
Infer field types from field values.
"""
directive @infer(cat: [SpaceProgram]) on OBJECT
"#
        );
    }
}
