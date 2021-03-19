use crate::Field;
use std::fmt::{self, Display};

/// A GraphQL service’s collective type system capabilities are referred to as that service’s “schema”.
///
/// *SchemaDefinition*:
///     Description<sub>opt</sub> **schema** Directives<sub>\[Const\] opt</sub> **{** RootOperationTypeDefinition<sub>list</sub> **}**
///
/// Detailed documentation can be found in [GraphQL spec](https://spec.graphql.org/draft/#sec-Schema).
///
/// ### Example
/// ```rust
/// use sdl_encoder::{FieldValue, Field, SchemaDef};
/// use indoc::indoc;
///
/// let ty_1 = FieldValue::Type {
///     ty: "TryingToFindCatQuery".to_string(),
/// };
///
/// let field = Field::new("query".to_string(), ty_1);
///
/// let mut schema_def = SchemaDef::new(field);
/// schema_def.description(Some("Root Schema".to_string()));
///
/// assert_eq!(
///     schema_def.to_string(),
///     indoc! { r#"
///         """Root Schema"""
///         schema {
///           query: TryingToFindCatQuery
///         }
///     "#}
/// );
/// ```

#[derive(Debug, Clone)]
pub struct SchemaDef {
    // Description may be a String.
    description: Option<String>,
    // The vector of fields in a schema to represent root operation type
    // definition.
    fields: Vec<Field>,
}

impl SchemaDef {
    /// Create a new instance of SchemaDef.
    pub fn new(field: Field) -> Self {
        Self {
            description: None,
            fields: vec![field],
        }
    }

    /// Set the SchemaDef's description.
    pub fn description(&mut self, description: Option<String>) {
        self.description = description
    }

    /// Push a Field to SchemaDef's fields vector.
    pub fn field(&mut self, field: Field) {
        self.fields.push(field)
    }
}

impl Display for SchemaDef {
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
        write!(f, "schema {{")?;
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
    fn it_encodes_schema_with_description() {
        let ty_1 = FieldValue::Type {
            ty: "TryingToFindCatQuery".to_string(),
        };

        let field = Field::new("query".to_string(), ty_1);

        let mut schema_def = SchemaDef::new(field);
        schema_def.description(Some("Root Schema".to_string()));

        assert_eq!(
            schema_def.to_string(),
            indoc! { r#"
                """Root Schema"""
                schema {
                  query: TryingToFindCatQuery
                }
            "#}
        );
    }
}
