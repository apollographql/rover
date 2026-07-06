use std::{collections::HashMap, fmt::Display, str::FromStr};

use apollo_parser::{
    cst::{Definition, OperationDefinition},
    Parser,
};
use serde::{
    de::{self, Deserializer},
    Deserialize, Serialize,
};

pub use crate::operations::persisted_queries::publish::runner::publish_operations_mutation::PublishOperationsMutationGraphPersistedQueryListPublishOperations as PersistedQueryPublishOperationResult;
use crate::{
    operations::persisted_queries::publish::runner::publish_operations_mutation::{
        self, OperationType as RoverClientOperationType, PersistedQueryInput,
    },
    RoverClientError,
};

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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct PersistedQueryOperation {
    pub name: String,
    pub r#type: PersistedQueryOperationType,
    pub body: String,
    pub id: String,
    pub client_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Ord, PartialOrd)]
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
                        client_name: operation.client_name,
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

impl TryFrom<RelayPersistedQueryManifest> for ApolloPersistedQueryManifest {
    type Error = RoverClientError;

    fn try_from(relay_manifest: RelayPersistedQueryManifest) -> Result<Self, Self::Error> {
        let mut anonymous_operations = Vec::new();
        let mut syntax_errors = Vec::new();
        let mut ids_with_multiple_operations = Vec::new();
        let mut ids_with_no_operations = Vec::new();
        let mut operations = Vec::new();
        for (id, body) in relay_manifest.operations {
            let ast = Parser::new(&body).parse();

            let operation_definitions: Vec<OperationDefinition> = ast
                .clone()
                .document()
                .definitions()
                .filter_map(|definition| {
                    if let Definition::OperationDefinition(operation_definition) = definition {
                        Some(operation_definition)
                    } else {
                        None
                    }
                })
                .collect();

            let maybe_definition = match &operation_definitions[..] {
                [operation_definition] => Some(operation_definition),
                [] => {
                    ids_with_no_operations.push(id.clone());
                    None
                }
                _ => {
                    ids_with_multiple_operations.push(id.clone());
                    None
                }
            };

            if let Some(operation_definition) = maybe_definition {
                // attempt to extract operation type, defaulting to "query" if we can't find one
                let operation_type = match operation_definition.operation_type() {
                    Some(operation_type) => {
                        match (
                            operation_type.mutation_token(),
                            operation_type.query_token(),
                            operation_type.subscription_token(),
                        ) {
                            (Some(_mutation), _, _) => PersistedQueryOperationType::Mutation,
                            (_, Some(_query), _) => PersistedQueryOperationType::Query,
                            (_, _, Some(_subscription)) => {
                                PersistedQueryOperationType::Subscription
                            }
                            // this should probably be unreachable, but just default to query regardless
                            _ => PersistedQueryOperationType::Query,
                        }
                    }
                    None => PersistedQueryOperationType::Query,
                };

                // track valid operations and the IDs of invalid operations
                if let Some(operation_name) = operation_definition.name() {
                    operations.push(PersistedQueryOperation {
                        name: operation_name.text().to_string(),
                        r#type: operation_type,
                        body: body.to_string(),
                        id: id.to_string(),
                        // Relay format has no way to include client names in
                        // the manifest file; you can still use the
                        // `--for-client-name` flag.
                        client_name: None,
                    });
                } else {
                    // `apollo-parser` may sometimes be able to detect an operation name
                    // even if there are syntax errors
                    // we only report syntax errors when the operation name cannot be detected
                    // to relax GraphQL parsing as much as possible
                    let mut parse_errors = ast.errors().peekable();
                    if parse_errors.peek().is_some() {
                        syntax_errors.push((
                            id.clone(),
                            parse_errors
                                .map(|err| err.to_string())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        ));
                    } else {
                        anonymous_operations.push(id.clone());
                    }
                }
            }
        }

        let mut errors = Vec::new();

        if !anonymous_operations.is_empty() {
            errors.push(format!(
                "The following operation IDs do not have a name: {}.",
                anonymous_operations.join(", ")
            ));
        }

        if !ids_with_multiple_operations.is_empty() {
            errors.push(format!(
                "The following operation IDs contained multiple operations: {}.",
                ids_with_multiple_operations.join(", ")
            ));
        }

        if !ids_with_no_operations.is_empty() {
            errors.push(format!(
                "The following operation IDs contained no operations: {}.",
                ids_with_no_operations.join(", ")
            ));
        }

        if !syntax_errors.is_empty() {
            for (id, syntax_errors) in syntax_errors {
                errors.push(format!("The operation with ID {id} contained the following syntax errors:\n\n{syntax_errors}"));
            }
        }

        if errors.is_empty() {
            let manifest = ApolloPersistedQueryManifest { operations };
            if let Ok(json) = serde_json::to_string(&manifest) {
                tracing::debug!(json, "successfully converted relay persisted query manifest to apollo persisted query manifest");
            }
            Ok(manifest)
        } else {
            Err(RoverClientError::RelayOperationParseFailures {
                errors: errors.join("\n"),
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

    pub const fn total(&self) -> i64 {
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
        let id = "ed145403db84d192c3f2f44eaa9bc6f9";
        let body = "query NewsfeedQuery {\n  topStory {\n    title\n    summary\n    poster {\n      __typename\n      name\n      profilePicture {\n        url\n      }\n      id\n    }\n    thumbnail {\n      url\n    }\n    id\n  }\n}\n";
        let relay_manifest = serde_json::json!({id: body}).to_string();

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(&relay_manifest).expect("could not read relay manifest");
        assert_eq!(relay_manifest.operations.len(), 1);
    }

    #[test]
    fn it_can_convert_relay_manifest_to_apollo_manifest() {
        let id = "ed145403db84d192c3f2f44eaa9bc6f9";
        let body = "query NewsfeedQuery {\n  topStory {\n    title\n    summary\n    poster {\n      __typename\n      name\n      profilePicture {\n        url\n      }\n      id\n    }\n    thumbnail {\n      url\n    }\n    id\n  }\n}\n";
        let relay_manifest = serde_json::json!({id: body}).to_string();

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(&relay_manifest).expect("could not read relay manifest");
        let apollo_manifest: ApolloPersistedQueryManifest = relay_manifest.try_into().unwrap();
        assert_eq!(
            apollo_manifest.operations[0],
            PersistedQueryOperation {
                name: "NewsfeedQuery".to_string(),
                r#type: PersistedQueryOperationType::Query,
                id: id.to_string(),
                body: body.to_string(),
                client_name: None,
            }
        );
    }

    #[test]
    fn it_can_convert_relay_manifest_with_fragment_to_apollo_manifest() {
        let id = "ed145403db84d192c3f2f44eaa9bc6f9";
        let body = "query NewsfeedQuery {\n  topStory {\n    title\n    summary\n    poster {\n      __typename\n      name\n      profilePicture {\n        url\n      }\n      id\n    }\n    thumbnail {\n      url\n    }\n    id\n  }\n}\n fragment AuthorDetails on Article { \n firstName \n lastName \n }";
        let relay_manifest = serde_json::json!({id: body}).to_string();

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(&relay_manifest).expect("could not read relay manifest");
        let apollo_manifest: ApolloPersistedQueryManifest = relay_manifest.try_into().unwrap();
        assert_eq!(
            apollo_manifest.operations[0],
            PersistedQueryOperation {
                name: "NewsfeedQuery".to_string(),
                r#type: PersistedQueryOperationType::Query,
                id: id.to_string(),
                body: body.to_string(),
                client_name: None,
            }
        );
    }

    #[test]
    fn it_can_convert_relay_manifest_with_multiple_queries_to_apollo_manifest() {
        let id_one = "ed145403db84d192c3f2f44eaa9bc6f9";
        let body_one = "query NewsfeedQuery {\n  topStory {\n    title\n    summary\n    poster {\n      __typename\n      name\n      profilePicture {\n        url\n      }\n      id\n    }\n    thumbnail {\n      url\n    }\n    id\n  }\n}\n";
        let id_two = "adkjflaskdjf";
        let body_two = "mutation NamedMutation { topStory }";
        let relay_manifest = serde_json::json!({id_one: body_one, id_two: body_two}).to_string();

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(&relay_manifest).expect("could not read relay manifest");
        let mut apollo_manifest: ApolloPersistedQueryManifest = relay_manifest.try_into().unwrap();

        // guarantee proper ordering
        apollo_manifest.operations.sort();

        assert_eq!(
            apollo_manifest.operations[0],
            PersistedQueryOperation {
                name: "NamedMutation".to_string(),
                r#type: PersistedQueryOperationType::Mutation,
                id: id_two.to_string(),
                client_name: None,
                body: body_two.to_string()
            }
        );

        assert_eq!(
            apollo_manifest.operations[1],
            PersistedQueryOperation {
                name: "NewsfeedQuery".to_string(),
                r#type: PersistedQueryOperationType::Query,
                id: id_one.to_string(),
                client_name: None,
                body: body_one.to_string()
            }
        );
    }

    #[test]
    fn relay_manifest_with_anonymous_operations_fails() {
        let id = "ed145403db84d192c3f2f44eaa9bc6f9";
        let body = "query {\n  topStory {\n    title\n    summary\n    poster {\n      __typename\n      name\n      profilePicture {\n        url\n      }\n      id\n    }\n    thumbnail {\n      url\n    }\n    id\n  }\n}\n";
        let relay_manifest = serde_json::json!({id: body}).to_string();

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(&relay_manifest).expect("could not read relay manifest");
        let apollo_manifest_result: Result<ApolloPersistedQueryManifest, RoverClientError> =
            relay_manifest.try_into();
        assert!(matches!(
            apollo_manifest_result,
            Err(RoverClientError::RelayOperationParseFailures { .. })
        ));
        let error = apollo_manifest_result.unwrap_err().to_string();
        assert!(error.contains(id));
        assert!(error.contains("do not have a name"));
    }

    #[test]
    fn relay_manifest_with_anonymous_operation_and_valid_operation_fails() {
        let id_one = "ed145403db84d192c3f2f44eaa9bc6f9";
        let body_one = "query {\n  topStory {\n    title\n    summary\n    poster {\n      __typename\n      name\n      profilePicture {\n        url\n      }\n      id\n    }\n    thumbnail {\n      url\n    }\n    id\n  }\n}\n";
        let id_two = "adkjflaskdjf";
        let body_two = "query NamedQuery { topStory }";
        let relay_manifest = serde_json::json!({id_one: body_one, id_two: body_two}).to_string();

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(&relay_manifest).expect("could not read relay manifest");
        let apollo_manifest_result: Result<ApolloPersistedQueryManifest, RoverClientError> =
            relay_manifest.try_into();
        assert!(matches!(
            apollo_manifest_result,
            Err(RoverClientError::RelayOperationParseFailures { .. })
        ));
        let error = apollo_manifest_result.unwrap_err().to_string();
        assert!(error.contains(id_one));
        assert!(error.contains("do not have a name"))
    }

    #[test]
    fn relay_manifest_with_invalid_operation_type_fails() {
        let id = "ed145403db84d192c3f2f44eaa9bc6f9";
        let body = "queryyyyy NewsfeedQuery {\n  topStory {\n    title\n    summary\n    poster {\n      __typename\n      name\n      profilePicture {\n        url\n      }\n      id\n    }\n    thumbnail {\n      url\n    }\n    id\n  }\n}\n";
        let relay_manifest = serde_json::json!({id: body}).to_string();

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(&relay_manifest).expect("could not read relay manifest");
        let apollo_manifest_result: Result<ApolloPersistedQueryManifest, RoverClientError> =
            relay_manifest.try_into();
        assert!(matches!(
            apollo_manifest_result,
            Err(RoverClientError::RelayOperationParseFailures { .. })
        ));
        let error = apollo_manifest_result.unwrap_err().to_string();
        assert!(error.contains(id));
        assert!(error.contains("syntax error"))
    }

    #[test]
    fn relay_manifest_with_multiple_operations_in_one_document_cannot_be_converted() {
        let id = "120931209";
        let body = "query FirstQuery { topStory } \n query SecondQuery { topStory { title} }";
        let relay_manifest = serde_json::json!({id: body}).to_string();

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(&relay_manifest).expect("could not read relay manifest");
        let apollo_manifest_result: Result<ApolloPersistedQueryManifest, RoverClientError> =
            relay_manifest.try_into();
        assert!(matches!(
            apollo_manifest_result,
            Err(RoverClientError::RelayOperationParseFailures { .. })
        ));
        let error = apollo_manifest_result.unwrap_err().to_string();
        assert!(error.contains(id));
        assert!(error.contains("multiple operations"))
    }

    #[test]
    fn relay_manifest_with_no_operations_in_one_document_cannot_be_converted() {
        let id = "120931209";
        let body = "type NewsFeed { topStory: String }";
        let relay_manifest = serde_json::json!({id: body}).to_string();

        let relay_manifest: RelayPersistedQueryManifest =
            serde_json::from_str(&relay_manifest).expect("could not read relay manifest");
        let apollo_manifest_result: Result<ApolloPersistedQueryManifest, RoverClientError> =
            relay_manifest.try_into();
        assert!(matches!(
            apollo_manifest_result,
            Err(RoverClientError::RelayOperationParseFailures { .. })
        ));
        let error = apollo_manifest_result.unwrap_err().to_string();
        assert!(error.contains(id));
        assert!(error.contains("no operations"))
    }
}
