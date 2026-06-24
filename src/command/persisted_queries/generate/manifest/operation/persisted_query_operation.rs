use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Serialize)]
pub(crate) struct PersistedQueryOperation {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(rename = "type")]
    pub(super) operation_type: &'static str,
    pub(super) body: String,
}

pub(super) fn sha256_hex(body: &str) -> String {
    Sha256::digest(body.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
