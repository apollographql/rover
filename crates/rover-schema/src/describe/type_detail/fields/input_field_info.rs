use apollo_compiler::{Name, schema::InputValueDefinition};

use crate::describe::deprecated::IsDeprecated;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct InputFieldInfo {
    pub name: Name,
    pub field_type: Name,
    pub description: Option<String>,
    pub is_deprecated: bool,
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
