use bon::Builder;
use graphql_client::GraphQLQuery;

pub mod service;

pub use service::DeletePair;

/// Deletes a `client_credentials` OAuth client (client-credential pair): a soft delete that
/// also removes its secrets, service account, and principal (spec FR28-30). The mutation
/// returns the deleted client's ID, which the caller already passed in - there's nothing else
/// to report.
#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/api_key/pair_delete/delete_pair_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    variables_derives = "Clone, PartialEq, Debug",
    deprecated = "warn"
)]
pub struct DeletePairMutation;

#[derive(Clone, Debug, Builder)]
pub struct DeletePairInput {
    #[builder(into)]
    pub organization_id: String,
    #[builder(into)]
    pub client_id: String,
}
