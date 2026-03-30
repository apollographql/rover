use itertools::Itertools;

use super::{expanded_type::ExpandedType, field_info::FieldInfo, fields_detail::FieldsDetail};
use crate::ParsedSchema;

/// Field listing for an object or interface, augmented with deprecation counts and type expansions.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtendedFieldsDetail {
    #[serde(flatten)]
    fields: FieldsDetail,
    /// Number of deprecated fields (always computed from all fields, regardless of filter).
    pub deprecated_count: usize,
    /// Inline expansions of types referenced by the visible fields, up to the requested depth.
    pub expanded_types: Vec<ExpandedType>,
}

impl ExtendedFieldsDetail {
    /// Construct an `ExtendedFieldsDetail` from its parts.
    pub const fn new(
        fields: FieldsDetail,
        deprecated_count: usize,
        expanded_types: Vec<ExpandedType>,
    ) -> Self {
        Self {
            fields,
            deprecated_count,
            expanded_types,
        }
    }

    /// Returns the visible fields (deprecated fields excluded when the filter was applied).
    pub fn fields(&self) -> &[FieldInfo] {
        self.fields.fields()
    }

    /// Returns the total field count including deprecated fields.
    pub const fn field_count(&self) -> usize {
        self.fields.field_count
    }
}

impl ParsedSchema {
    pub(in crate::describe::type_detail) fn extended_fields_detail(
        &self,
        all_fields: Vec<FieldInfo>,
        include_deprecated: bool,
        depth: usize,
    ) -> ExtendedFieldsDetail {
        let deprecated_count = all_fields.iter().filter(|f| f.is_deprecated).count();
        let field_count = all_fields.len();
        let fields = if include_deprecated {
            all_fields
        } else {
            all_fields
                .into_iter()
                .filter(|f| !f.is_deprecated)
                .collect()
        };
        let expanded_types = if depth > 0 {
            self.expand_referenced_types(&fields, depth, include_deprecated)
        } else {
            Vec::new()
        };
        ExtendedFieldsDetail::new(
            FieldsDetail::new(fields, field_count),
            deprecated_count,
            expanded_types,
        )
    }

    fn expand_referenced_types(
        &self,
        fields: &[FieldInfo],
        depth: usize,
        include_deprecated: bool,
    ) -> Vec<ExpandedType> {
        if depth == 0 {
            return Vec::new();
        }
        let schema = self.inner();
        fields
            .iter()
            .filter(|f| {
                schema
                    .types
                    .get(f.return_type.as_str())
                    .is_some_and(|ty| !ty.is_built_in())
            })
            .unique_by(|f| &f.return_type)
            .filter_map(|f| self.expand_single_type(f.return_type.as_str(), include_deprecated))
            .collect()
    }
}
