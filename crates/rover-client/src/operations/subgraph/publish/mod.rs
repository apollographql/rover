mod runner;
mod service;
mod types;

use graphql_client::GraphQLQuery;
pub use runner::run;
pub use types::{SubgraphPublishInput, SubgraphPublishResponse};

type Timestamp = String;

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/subgraph/publish/launch_status_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
/// A lightweight poll query for a launch (and its downstream contract-variant
/// launches) triggered by a `subgraph publish`.
pub(crate) struct SubgraphPublishLaunchStatusQuery;
