use apollo_compiler::{Name, schema::ObjectType};

use super::fields::{ExtendedFieldsDetail, FieldInfo};
use crate::{ParsedSchema, root_paths::RootPath};

/// Detailed view of a GraphQL object type.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ObjectDetail {
    /// The type name.
    pub name: Name,
    /// Optional description from the schema SDL.
    pub description: Option<String>,
    /// Interfaces this object implements.
    pub implements: Vec<Name>,
    /// Fields defined on this type, including deprecation and expansion info.
    #[serde(flatten)]
    pub fields: ExtendedFieldsDetail,
    /// Root paths from Query/Mutation to this type.
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
        ObjectDetail {
            name: type_name.clone(),
            description,
            implements,
            fields,
            via,
        }
    }
}
