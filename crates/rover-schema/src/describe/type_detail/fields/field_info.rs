use apollo_compiler::{Name, schema::FieldDefinition};

use crate::describe::deprecated::IsDeprecated;

/// Summary metadata for a field, used in type-level field listings.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FieldInfo {
    /// The field name.
    pub name: Name,
    /// The inner named return type.
    pub return_type: Name,
    /// Optional description from the schema SDL.
    pub description: Option<String>,
    /// Whether this field is marked `@deprecated`.
    pub is_deprecated: bool,
    /// The reason given for deprecation, if any.
    pub deprecation_reason: Option<String>,
    /// Number of arguments this field accepts.
    pub arg_count: usize,
}

impl FieldInfo {
    pub(crate) fn from_field_definition(name: Name, field: &FieldDefinition) -> Self {
        Self {
            name,
            return_type: field.ty.inner_named_type().clone(),
            description: field.description.as_ref().map(|d| d.to_string()),
            is_deprecated: field.is_deprecated(),
            deprecation_reason: field.deprecation_reason(),
            arg_count: field.arguments.len(),
        }
    }
}
