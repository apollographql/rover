use std::{fmt::Display, str::FromStr};

use serde::{
    de::{self, Deserializer},
    Deserialize, Serialize,
};

use crate::{
    operations::persisted_queries::publish::runner::publish_operations_mutation::{
        self, OperationType, PersistedQueryInput,
    },
    RoverClientError,
};

pub use crate::operations::persisted_queries::publish::runner::publish_operations_mutation::PublishOperationsMutationGraphPersistedQueryListPublishOperations as PersistedQueryPublishOperationResult;

type QueryVariables = publish_operations_mutation::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PersistedQueriesPublishInput {
    pub graph_id: String,
    pub list_id: String,
    pub operation_manifest: PersistedQueryManifest,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistedQueryManifest {
    operations: Vec<PersistedQueryOperation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PersistedQueryOperation {
    pub name: String,
    pub r#type: PersistedQueryOperationType,
    pub body: String,
    pub id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum PersistedQueryOperationType {
    Query,
    Mutation,
    Subscription,
}

impl FromStr for PersistedQueryOperationType {
    type Err = RoverClientError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "query" => Ok(Self::Query),
            "mutation" => Ok(Self::Mutation),
            "subscription" => Ok(Self::Subscription),
            input => Err(RoverClientError::AdhocError { msg: format!("'{input}' is not a valid operation type. Must be one of: 'query', 'mutation', or 'subscription'.") })
        }
    }
}

impl<'de> Deserialize<'de> for PersistedQueryOperationType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        FromStr::from_str(&s).map_err(de::Error::custom)
    }
}

impl Display for PersistedQueryOperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let result = match &self {
            Self::Query => "QUERY",
            Self::Mutation => "MUTATION",
            Self::Subscription => "SUBSCRIPTION",
        };

        write!(f, "{result}")
    }
}

impl From<PersistedQueriesPublishInput> for QueryVariables {
    fn from(input: PersistedQueriesPublishInput) -> Self {
        Self {
            graph_id: input.graph_id,
            list_id: input.list_id,
            operation_manifest: Some(
                input
                    .operation_manifest
                    .operations
                    .iter()
                    .cloned()
                    .map(|operation| PersistedQueryInput {
                        name: operation.name,
                        body: operation.body,
                        id: operation.id,
                        type_: match operation.r#type {
                            PersistedQueryOperationType::Mutation => OperationType::MUTATION,
                            PersistedQueryOperationType::Subscription => {
                                OperationType::SUBSCRIPTION
                            }
                            PersistedQueryOperationType::Query => OperationType::QUERY,
                        },
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedQueriesPublishResponse {
    pub revision: i64,
    pub graph_id: String,
    pub list_id: String,
    pub result: PersistedQueriesPublishResponseType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PersistedQueriesPublishResponseType {
    New(PersistedQueriesPublishResponseNewRevision),
    Unchanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedQueriesPublishResponseNewRevision {
    pub added: i64,
    pub identical: i64,
    pub removed: i64,
    pub unaffected: i64,
    pub updated: i64,
}

impl PersistedQueriesPublishResponseNewRevision {
    pub fn added_str(&self) -> Option<String> {
        Self::ops_str(self.added)
    }

    pub fn updated_str(&self) -> Option<String> {
        Self::ops_str(self.updated)
    }

    pub fn removed_str(&self) -> Option<String> {
        Self::ops_str(self.removed)
    }

    fn ops_str(n: i64) -> Option<String> {
        match n {
            0 => None,
            1 => Some(format!("1 operation")),
            n => Some(format!("{n} operations")),
        }
    }
}
