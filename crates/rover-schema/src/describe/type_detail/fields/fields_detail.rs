use super::field_info::FieldInfo;

/// A list of fields together with a pre-computed total count.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldsDetail {
    fields: Vec<FieldInfo>,
    /// Total number of fields, including any that were filtered out.
    pub field_count: usize,
}

impl FieldsDetail {
    /// Construct a `FieldsDetail` from a field list and the pre-computed total count.
    pub const fn new(fields: Vec<FieldInfo>, field_count: usize) -> Self {
        Self {
            fields,
            field_count,
        }
    }

    /// Returns the (possibly filtered) field slice.
    pub fn fields(&self) -> &[FieldInfo] {
        &self.fields
    }
}
