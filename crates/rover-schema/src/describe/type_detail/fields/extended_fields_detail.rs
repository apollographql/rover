use crate::ParsedSchema;

use super::expanded_type::ExpandedType;
use super::field_info::FieldInfo;
use super::fields_detail::FieldsDetail;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExtendedFieldsDetail {
    #[serde(flatten)]
    fields: FieldsDetail,
    pub deprecated_count: usize,
    pub expanded_types: Vec<ExpandedType>,
}

impl ExtendedFieldsDetail {
    pub fn new(fields: FieldsDetail, deprecated_count: usize, expanded_types: Vec<ExpandedType>) -> Self {
        Self { fields, deprecated_count, expanded_types }
    }

    pub fn fields(&self) -> &[FieldInfo] {
        self.fields.fields()
    }

    pub fn field_count(&self) -> usize {
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
            all_fields.into_iter().filter(|f| !f.is_deprecated).collect()
        };
        let expanded_types = if depth > 0 {
            self.expand_referenced_types(&fields, depth, include_deprecated)
        } else {
            Vec::new()
        };
        ExtendedFieldsDetail::new(FieldsDetail::new(fields, field_count), deprecated_count, expanded_types)
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
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for field in fields {
            if self
                .inner()
                .types
                .get(field.return_type.as_str())
                .map_or(true, |ty| ty.is_built_in())
            {
                continue;
            }
            if seen.contains(&field.return_type) {
                continue;
            }
            seen.insert(field.return_type.clone());
            if let Some(expanded) = self.expand_single_type(field.return_type.as_str(), include_deprecated) {
                result.push(expanded);
            }
        }
        result
    }
}
