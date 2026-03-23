use apollo_compiler::{Name, schema::InterfaceType};

use super::fields::{ExtendedFieldsDetail, FieldInfo};
use crate::{ParsedSchema, root_paths::RootPath};

#[derive(Debug, Clone, serde::Serialize)]
pub struct InterfaceDetail {
    pub name: Name,
    pub description: Option<String>,
    pub implements: Vec<Name>,
    #[serde(flatten)]
    pub fields: ExtendedFieldsDetail,
    pub implementors: Vec<Name>,
    pub via: Vec<RootPath>,
}

impl ParsedSchema {
    pub fn find_implementors(&self, interface_name: &Name) -> Vec<Name> {
        self.inner()
            .implementers_map()
            .get(interface_name)
            .map(|imp| imp.objects.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub(super) fn build_interface_detail(
        &self,
        type_name: &Name,
        iface: &InterfaceType,
        include_deprecated: bool,
        depth: usize,
    ) -> InterfaceDetail {
        let description = iface.description.as_ref().map(|d| d.to_string());
        let implements = iface
            .implements_interfaces
            .iter()
            .map(|i| i.name.clone())
            .collect();
        let all_fields: Vec<FieldInfo> = iface
            .fields
            .iter()
            .map(|(n, field)| FieldInfo::from_field_definition(n.clone(), field))
            .collect();
        let fields = self.extended_fields_detail(all_fields, include_deprecated, depth);
        let implementors = self.find_implementors(type_name);
        let via = self.find_root_paths(type_name);
        InterfaceDetail {
            name: type_name.clone(),
            description,
            implements,
            fields,
            implementors,
            via,
        }
    }
}
