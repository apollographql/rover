use apollo_compiler::{Name, Schema, ast::Type as AstType, schema::ExtendedType};

/// A lightweight summary of a field used in schema overview root-type listings.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldSummary {
    /// The field name.
    pub name: Name,
    /// The full return type.
    pub return_type: AstType,
}

impl FieldSummary {
    pub(crate) fn new(schema: &Schema, root_name: &str) -> Vec<Self> {
        if let Some(ExtendedType::Object(obj)) = schema.types.get(root_name) {
            obj.fields
                .iter()
                .map(|(name, field)| FieldSummary {
                    name: name.clone(),
                    return_type: field.ty.clone(),
                })
                .collect()
        } else {
            Vec::new()
        }
    }
}
