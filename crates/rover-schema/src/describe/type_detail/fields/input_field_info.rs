use apollo_compiler::{Name, schema::InputValueDefinition};

use crate::describe::deprecated::IsDeprecated;

/// Metadata for a single field within an input object type.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct InputFieldInfo {
    /// The input field name.
    pub name: Name,
    /// The inner named type of the field.
    pub field_type: Name,
    /// Optional description from the schema SDL.
    pub description: Option<String>,
    /// Whether this field is marked `@deprecated`.
    pub is_deprecated: bool,
    /// The reason given for deprecation, if any.
    pub deprecation_reason: Option<String>,
}

impl InputFieldInfo {
    pub(crate) fn from_input_value_definition(name: Name, field: &InputValueDefinition) -> Self {
        Self {
            name,
            field_type: field.ty.inner_named_type().clone(),
            description: field.description.as_ref().map(|d| d.to_string()),
            is_deprecated: field.is_deprecated(),
            deprecation_reason: field.deprecation_reason(),
        }
    }
}
