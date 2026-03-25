use apollo_compiler::{Name, schema::EnumType};

use super::fields::EnumValueInfo;
use crate::{ParsedSchema, describe::deprecated::IsDeprecated, root_paths::RootPath};

/// Detailed view of a GraphQL enum type.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnumDetail {
    /// The enum type name.
    pub name: Name,
    /// Optional description from the schema SDL.
    pub description: Option<String>,
    /// The enum values (filtered by `include_deprecated` when built).
    pub values: Vec<EnumValueInfo>,
    /// Total number of enum values including deprecated ones.
    pub value_count: usize,
    /// Number of deprecated enum values.
    pub deprecated_count: usize,
    /// Root paths from Query/Mutation to this type.
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
