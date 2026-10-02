use bon::Builder;
use graphql_client::GraphQLQuery;

pub mod service;

pub use service::DeletePair;

/// `deleteOAuthClient` returns nothing to read back on success. The schema documents its `Void`
/// result as "always null", but the Platform API has been seen returning a non-null value for a
/// successful call, which `()` can only reject - reporting a change that took effect as a failure.
/// Any value is accepted, and ignored.
type Void = serde_json::Value;

/// Deletes a `client_credentials` OAuth client (client-credential pair): a soft delete that
/// also removes its secrets, service account, and principal (spec FR28-30). The caller already
/// knows the `client_id` it passed in - there's nothing else to report.
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
