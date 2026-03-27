use rover_studio::types::GraphRef;
use serde::{Deserialize, Serialize};

use crate::shared::GitContext;

#[derive(Debug, Clone, Serialize)]
pub struct ValidateOperationsInput {
    pub graph_ref: GraphRef,
    pub operations: Vec<OperationDocument>,
    pub git_context: GitContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationDocument {
    pub name: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub operation_name: String,
    pub r#type: String,
    pub code: Option<String>,
    pub description: String,
}
