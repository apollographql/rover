use crate::operations::graph::validate_operations::validate_operations_query;
use crate::shared::GitContext;
use crate::shared::GraphRef;
use serde::{Deserialize, Serialize};

type QueryVariables = validate_operations_query::Variables;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl From<ValidateOperationsInput> for QueryVariables {
    fn from(input: ValidateOperationsInput) -> Self {
        Self {
            id: input.graph_ref.name,
            tag: input.graph_ref.variant,
            operations: input
                .operations
                .into_iter()
                .map(|op| validate_operations_query::OperationDocumentInput {
                    name: op.name,
                    body: op.body,
                })
                .collect(),
            git_context: Some(validate_operations_query::GitContextInput {
                branch: input.git_context.branch,
                commit: input.git_context.commit,
                committer: input.git_context.author,
                message: None,
                remote_url: input.git_context.remote_url,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub operation_name: String,
    pub r#type: String,
    pub code: Option<String>,
    pub description: String,
}

impl From<validate_operations_query::ValidateOperationsQueryServiceValidateOperationsValidationResults> for ValidationResult {
    fn from(result: validate_operations_query::ValidateOperationsQueryServiceValidateOperationsValidationResults) -> Self {
        Self {
            operation_name: result.operation.and_then(|o| o.name).unwrap_or_default(),
            r#type: result.r#type,
            code: result.code,
            description: result.description.unwrap_or_default(),
        }
    }
}
