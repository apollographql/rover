use apollo_compiler::{Name, schema::InputObjectType};

use crate::{ParsedSchema, root_paths::RootPath};

use super::fields::{FieldInfo, FieldsDetail};

#[derive(Debug, Clone, serde::Serialize)]
pub struct InputDetail {
    pub name: Name,
    pub description: Option<String>,
    #[serde(flatten)]
    pub fields: FieldsDetail,
    pub via: Vec<RootPath>,
}

impl ParsedSchema {
    pub(super) fn build_input_detail(
        &self,
        type_name: &Name,
        inp: &InputObjectType,
    ) -> InputDetail {
        let description = inp.description.as_ref().map(|d| d.to_string());
        let fields: Vec<FieldInfo> = inp
            .fields
            .iter()
            .map(|(n, field)| FieldInfo::from_input_value_definition(n.clone(), field))
            .collect();
        let field_count = fields.len();
        let via = self.find_root_paths(type_name);
        InputDetail {
            name: type_name.clone(),
            description,
            fields: FieldsDetail::new(fields, field_count),
            via,
        }
    }
}
