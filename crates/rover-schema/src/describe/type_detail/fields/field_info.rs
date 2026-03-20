use apollo_compiler::{Name, schema::{FieldDefinition, InputValueDefinition}};

use crate::describe::deprecated::IsDeprecated;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FieldInfo {
    pub name: Name,
    pub return_type: Name,
    pub description: Option<String>,
    pub is_deprecated: bool,
    pub deprecation_reason: Option<String>,
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

    pub(crate) fn from_input_value_definition(name: Name, field: &InputValueDefinition) -> Self {
        Self {
            name,
            return_type: field.ty.inner_named_type().clone(),
            description: field.description.as_ref().map(|d| d.to_string()),
            is_deprecated: field.is_deprecated(),
            deprecation_reason: field.deprecation_reason(),
            arg_count: 0,
        }
    }
}
