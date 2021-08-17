use std::fmt::{self, Display};

use crate::Description;

/// Represents scalar types such as Int, String, and Boolean.
/// Scalars cannot have fields.
///
/// *ScalarTypeDefinition*:
///     Description<sub>opt</sub> **scalar** Name Directives<sub>\[Const\]opt</sub>
///
/// Detailed documentation can be found in [GraphQL spec](https://spec.graphql.org/draft/#sec-Scalar).
/// ### Example
/// ```rust
/// use sdl_encoder::ScalarDef;
///
/// let mut scalar = ScalarDef::new("NumberOfTreatsPerDay".to_string());
/// scalar.description(Some(
///     "Int representing number of treats received.".to_string(),
/// ));
///
/// assert_eq!(
///     scalar.to_string(),
///     r#""""Int representing number of treats received."""
/// scalar NumberOfTreatsPerDay
/// "#
/// );
/// ```
#[derive(Debug, PartialEq, Clone)]
pub struct ScalarDef {
    // Name must return a String.
    name: String,
    // Description may return a String or null.
    description: Description,
}

impl ScalarDef {
    /// Create a new instance of Scalar Definition.
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: Description::Top { source: None },
        }
    }

    /// Set the ScalarDef's description.
    pub fn description(&mut self, description: Option<String>) {
        self.description = Description::Top {
            source: description,
        };
    }
}

impl Display for ScalarDef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description)?;
        writeln!(f, "scalar {}", self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn it_encodes_scalar() {
        let scalar = ScalarDef::new("NumberOfTreatsPerDay".to_string());
        assert_eq!(
            scalar.to_string(),
            r#"scalar NumberOfTreatsPerDay
"#
        );
    }

    #[test]
    fn it_encodes_scalar_with_description() {
        let mut scalar = ScalarDef::new("NumberOfTreatsPerDay".to_string());
        scalar.description(Some(
            "Int representing number of treats received.".to_string(),
        ));

        assert_eq!(
            scalar.to_string(),
            r#""""Int representing number of treats received."""
scalar NumberOfTreatsPerDay
"#
        );
    }
}
