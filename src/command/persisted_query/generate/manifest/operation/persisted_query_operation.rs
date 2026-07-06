use serde::Serialize;

#[derive(Debug, Serialize)]
pub(crate) struct PersistedQueryOperation {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(rename = "type")]
    pub(super) operation_type: &'static str,
    pub(super) body: String,
}
