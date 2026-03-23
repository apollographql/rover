use apollo_compiler::{Name, schema::EnumType};

use super::fields::EnumValueInfo;
use crate::{ParsedSchema, describe::deprecated::IsDeprecated, root_paths::RootPath};

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnumDetail {
    pub name: Name,
    pub description: Option<String>,
    pub values: Vec<EnumValueInfo>,
    pub value_count: usize,
    pub deprecated_count: usize,
    pub via: Vec<RootPath>,
}

impl ParsedSchema {
    pub(super) fn build_enum_detail(
        &self,
        type_name: &Name,
        e: &EnumType,
        include_deprecated: bool,
    ) -> EnumDetail {
        let description = e.description.as_ref().map(|d| d.to_string());
        let all_values: Vec<EnumValueInfo> = e
            .values
            .iter()
            .map(|(n, val)| EnumValueInfo {
                name: n.clone(),
                description: val.description.as_ref().map(|d| d.to_string()),
                is_deprecated: val.is_deprecated(),
                deprecation_reason: val.deprecation_reason(),
            })
            .collect();
        let value_count = all_values.len();
        let deprecated_count = all_values.iter().filter(|v| v.is_deprecated).count();
        let values = if include_deprecated {
            all_values
        } else {
            all_values
                .into_iter()
                .filter(|v| !v.is_deprecated)
                .collect()
        };
        let via = self.find_root_paths(type_name);
        EnumDetail {
            name: type_name.clone(),
            description,
            values,
            value_count,
            deprecated_count,
            via,
        }
    }
}
