use apollo_compiler::{Name, schema::ObjectType};

use crate::{ParsedSchema, root_paths::RootPath};

use super::fields::{ExtendedFieldsDetail, FieldInfo};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ObjectDetail {
    pub name: Name,
    pub description: Option<String>,
    pub implements: Vec<Name>,
    #[serde(flatten)]
    pub fields: ExtendedFieldsDetail,
    pub via: Vec<RootPath>,
}

impl ParsedSchema {
    pub(super) fn build_object_detail(
        &self,
        type_name: &Name,
        obj: &ObjectType,
        include_deprecated: bool,
        depth: usize,
    ) -> ObjectDetail {
        let description = obj.description.as_ref().map(|d| d.to_string());
        let implements = obj
            .implements_interfaces
            .iter()
            .map(|i| i.name.clone())
            .collect();
        let all_fields: Vec<FieldInfo> = obj
            .fields
            .iter()
            .map(|(n, field)| FieldInfo::from_field_definition(n.clone(), field))
            .collect();
        let fields = self.extended_fields_detail(all_fields, include_deprecated, depth);
        let via = self.find_root_paths(type_name);
        ObjectDetail { name: type_name.clone(), description, implements, fields, via }
    }
}
