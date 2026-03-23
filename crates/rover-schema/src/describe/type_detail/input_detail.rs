use apollo_compiler::{Name, schema::InputObjectType};

use super::fields::InputFieldInfo;
use crate::{ParsedSchema, root_paths::RootPath};

#[derive(Debug, Clone, serde::Serialize)]
pub struct InputDetail {
    pub name: Name,
    pub description: Option<String>,
    pub field_count: usize,
    pub fields: Vec<InputFieldInfo>,
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
