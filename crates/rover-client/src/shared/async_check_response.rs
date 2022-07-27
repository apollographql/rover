use serde::Serialize;
use serde_json::{json, Value};

/// CheckRequestSuccessResult is the return type of the
/// `graph` and `subgraph` async check operations

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct CheckRequestSuccessResult {
    pub target_url: String,
    pub workflow_id: String,
}

impl CheckRequestSuccessResult {
    pub fn get_json(&self) -> Value {
        json!({
            "target_url": self.target_url,
            "workflow_id": self.workflow_id,
        })
    }
}
