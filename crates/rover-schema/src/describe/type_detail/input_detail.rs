use apollo_compiler::{Name, schema::InputObjectType};

use super::fields::InputFieldInfo;
use crate::{ParsedSchema, root_paths::RootPath};

/// Detailed view of a GraphQL input object type.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InputDetail {
    /// The input type name.
    pub name: Name,
    /// Optional description from the schema SDL.
    pub description: Option<String>,
    /// Total number of input fields.
    pub field_count: usize,
    /// The input fields defined on this type.
    pub fields: Vec<InputFieldInfo>,
    /// Root paths from Query/Mutation to this type via argument usage.
    pub via: Vec<RootPath>,
}

impl ParsedSchema {
    pub(super) fn build_input_detail(
        &self,
        type_name: &Name,
        inp: &InputObjectType,
    ) -> InputDetail {
        let description = inp.description.as_ref().map(|d| d.to_string());
        let fields: Vec<InputFieldInfo> = inp
            .fields
            .iter()
            .map(|(n, field)| InputFieldInfo::from_input_value_definition(n.clone(), field))
            .collect();
        let via = self.find_root_paths(type_name);
        InputDetail {
            name: type_name.clone(),
            description,
            field_count: fields.len(),
            fields,
            via,
        }
    }
}
