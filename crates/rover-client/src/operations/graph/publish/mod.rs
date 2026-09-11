mod runner;
mod service;
mod types;

use graphql_client::GraphQLQuery;
pub use runner::run;
pub use types::{
    ChangeSummary, FieldChanges, GraphPublishInput, GraphPublishResponse, TypeChanges,
};

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/graph/publish/launch_status_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
/// A lightweight poll query for a launch (and its downstream contract-variant
/// launches) triggered by a `graph publish`.
pub(crate) struct GraphPublishLaunchStatusQuery;
