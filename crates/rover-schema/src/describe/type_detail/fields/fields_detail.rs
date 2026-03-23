use super::field_info::FieldInfo;

#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldsDetail {
    fields: Vec<FieldInfo>,
    pub field_count: usize,
}

impl FieldsDetail {
    pub const fn new(fields: Vec<FieldInfo>, field_count: usize) -> Self {
        Self {
            fields,
            field_count,
        }
    }

    pub fn fields(&self) -> &[FieldInfo] {
        &self.fields
    }
}
