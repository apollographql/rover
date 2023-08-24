use std::{collections::HashMap, fmt::Display, str::FromStr};

use apollo_parser::{ast, Parser};
use ariadne::{Report, ReportKind};
use serde::{
    de::{self, Deserializer},
    Deserialize, Serialize,
};

use crate::{
    operations::persisted_queries::publish::runner::publish_operations_mutation::{
        self, OperationType as RoverClientOperationType, PersistedQueryInput,
    },
    RoverClientError,
};

pub use crate::operations::persisted_queries::publish::runner::publish_operations_mutation::PublishOperationsMutationGraphPersistedQueryListPublishOperations as PersistedQueryPublishOperationResult;

type QueryVariables = publish_operations_mutation::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PersistedQueriesPublishInput {
    pub graph_id: String,
    pub list_id: String,
    pub operation_manifest: ApolloPersistedQueryManifest,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ApolloPersistedQueryManifest {
    pub operations: Vec<PersistedQueryOperation>,
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
            input => Err(RoverClientError::AdhocError { msg: format!("'{input}' is not a valid operation type. Must be one of: 'QUERY', 'MUTATION', or 'SUBSCRIPTION'.") })
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
                            PersistedQueryOperationType::Mutation => {
                                RoverClientOperationType::MUTATION
                            }
                            PersistedQueryOperationType::Subscription => {
                                RoverClientOperationType::SUBSCRIPTION
                            }
                            PersistedQueryOperationType::Query => RoverClientOperationType::QUERY,
                        },
                    })
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RelayPersistedQueryManifest {
    #[serde(flatten)]
    operations: HashMap<String, String>,
}

impl TryFrom<(&str, RelayPersistedQueryManifest)> for ApolloPersistedQueryManifest {
    type Error = RoverClientError;

    fn try_from(
        path_and_manifest: (&str, RelayPersistedQueryManifest),
    ) -> Result<Self, Self::Error> {
        let (path, relay_manifest) = path_and_manifest;
        let mut error_reports = Vec::new();
        let mut operations = Vec::new();
        for (id, body) in relay_manifest.operations {
            let ast = Parser::new(&body).parse();
            error_reports.extend(ast.errors().map(|error| {
                // TODO: fix report
                Report::build(ReportKind::Error, path, error.index())
                    .with_message(error.message())
                    .finish()
            }));

            for definition in ast.document().definitions() {
                let mut operation_type = None;
                let mut operation_name = None;
                if let ast::Definition::OperationDefinition(operation_definition) = definition {
                    let operation_name = if let Some(name) = operation_definition.name() {
                        Some(name)
                    } else {
                        error_reports.push(
                            // TODO: fix report
                            Report::build(ReportKind::Error, path, 0)
                                .with_message("Operation does not have a name")
                                .finish(),
                        );
                        None
                    };
                    let operation_type = if let Some(ty) = operation_definition.operation_type() {
                        Some(ty)
                    } else {
                        // TODO: fix report
                        error_reports.push(Report::build(ReportKind::Error, path, 0).with_message("Operation does not have a valid type, must be one of: query, mutation, or subscription").finish());
                        None
                    };
                    if let (Some(operation_name), Some(operation_type)) =
                        (operation_name, operation_type)
                    {
                        operations.push(PersistedQueryOperation {
                            name: operation_name.text().to_string(),
                            r#type: match (operation_type.mutation_token(), operation_type.query_token(), operation_type.subscription_token()) {
                                (Some(_mutation), _, _) => PersistedQueryOperationType::Mutation,
                                (_, Some(_query), _) => PersistedQueryOperationType::Query,
                                (_, _, Some(_subscription)) => PersistedQueryOperationType::Subscription,
                                _ => unreachable!("operation type must be one of query, mutation, or subscription")
                            },
                            body: body.to_string(),
                            id: id.to_string()
                        });
                    }
                }
            }
        }

        if error_reports.is_empty() {
            Ok(ApolloPersistedQueryManifest { operations })
        } else {
            Err(RoverClientError::RelayOperationParseFailures {
                errors: error_reports
                    .iter()
                    .map(|d| {
                        let mut buf = std::io::Cursor::new(Vec::<u8>::new());
                        // TODO: figure out how to set up a cache so the report can be converted to a string
                        d.write(cache.as_ref(), &mut buf).unwrap();
                        std::str::from_utf8(&buf.into_inner()).unwrap().to_string()
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
            })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedQueriesPublishResponse {
    pub revision: i64,
    pub graph_id: String,
    pub list_id: String,
    pub list_name: String,
    pub total_published_operations: usize,
    pub unchanged: bool,
    pub operation_counts: PersistedQueriesOperationCounts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistedQueriesOperationCounts {
    pub added: i64,
    pub identical: i64,
    pub removed: i64,
    pub unaffected: i64,
    pub updated: i64,
}

impl PersistedQueriesOperationCounts {
    pub fn added_str(&self) -> Option<String> {
        Self::ops_str(self.added)
    }

    pub fn updated_str(&self) -> Option<String> {
        Self::ops_str(self.updated)
    }

    pub fn removed_str(&self) -> Option<String> {
        Self::ops_str(self.removed)
    }

    pub fn total(&self) -> i64 {
        self.added + self.identical + self.unaffected + self.updated - self.removed
    }

    fn ops_str(n: i64) -> Option<String> {
        match n {
            0 => None,
            1 => Some("1 operation".to_string()),
            n => Some(format!("{n} operations")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_can_read_relay_manifest() {
        let relay_manifest = r#"{
      "ed145403db84d192c3f2f44eaa9bc6f9": "query NewsfeedQuery {\n  topStory {\n    title\n    summary\n    poster {\n      __typename\n      name\n      profilePicture {\n        url\n      }\n      id\n    }\n    thumbnail {\n      url\n    }\n    id\n  }\n}\n"
    }"#;

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(relay_manifest).expect("could not read relay manifest");
        assert_eq!(relay_manifest.operations.len(), 1);
    }

    #[test]
    fn it_can_convert_relay_manifest_to_apollo_manifest() {
        let relay_manifest = r#"{
      "ed145403db84d192c3f2f44eaa9bc6f9": "query NewsfeedQuery {\n  topStory {\n    title\n    summary\n    poster {\n      __typename\n      name\n      profilePicture {\n        url\n      }\n      id\n    }\n    thumbnail {\n      url\n    }\n    id\n  }\n}\n"
    }"#;

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(relay_manifest).expect("could not read relay manifest");
        let apollo_manifest: ApolloPersistedQueryManifest =
            ("./persisted_query_manifest.json", relay_manifest)
                .try_into()
                .unwrap();
        assert_eq!(apollo_manifest.operations.len(), 1);
    }

    #[test]
    fn relay_manifest_with_anonymous_operations_fails() {
        let relay_manifest = r#"{
      "ed145403db84d192c3f2f44eaa9bc6f9": "query {\n  topStory {\n    title\n    summary\n    poster {\n      __typename\n      name\n      profilePicture {\n        url\n      }\n      id\n    }\n    thumbnail {\n      url\n    }\n    id\n  }\n}\n"
    }"#;

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(relay_manifest).expect("could not read relay manifest");
        let apollo_manifest_result: Result<ApolloPersistedQueryManifest, RoverClientError> =
            ("./persisted_query_manifest.json", relay_manifest).try_into();
        assert!(matches!(
            apollo_manifest_result,
            Err(RoverClientError::RelayOperationParseFailures { .. })
        ));
    }

    #[test]
    fn relay_manifest_with_invalid_operation_type_fails() {
        let relay_manifest = r#"{
      "ed145403db84d192c3f2f44eaa9bc6f9": "queryyy NewsfeedQuery {\n  topStory {\n    title\n    summary\n    poster {\n      __typename\n      name\n      profilePicture {\n        url\n      }\n      id\n    }\n    thumbnail {\n      url\n    }\n    id\n  }\n}\n"
    }"#;

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(relay_manifest).expect("could not read relay manifest");
        let apollo_manifest_result: Result<ApolloPersistedQueryManifest, RoverClientError> =
            ("./persisted_query_manifest.json", relay_manifest).try_into();
        assert!(matches!(
            apollo_manifest_result,
            Err(RoverClientError::RelayOperationParseFailures { .. })
        ));
    }
}
