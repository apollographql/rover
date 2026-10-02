use bon::Builder;
use graphql_client::GraphQLQuery;

pub mod service;

pub use service::{RevokeUserGrants, REVOKE_USER_GRANTS_ATTEMPT_TIMEOUT};

/// `revokeUserOAuthTokens` returns nothing to read back on success. The schema documents its `Void`
/// result as "always null", but the Platform API has been seen returning a non-null value for a
/// successful call, which `()` can only reject - reporting a change that took effect as a failure.
/// Any value is accepted, and ignored.
type Void = serde_json::Value;

/// Revokes every grant (refresh token) one user holds under one OAuth client, in an
/// organization - one step of `rover auth grants revoke`'s per-user sweep (spec FR55-FR66,
/// `specs/rover-431-identity-grant-management`). Outstanding access tokens aren't revoked; they
/// expire on their own. Revoking under a client where the user holds no grant succeeds, which
/// is what makes a retried sweep safe (FR64).
#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/auth/revoke_user_grants/revoke_user_grants_mutation.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    variables_derives = "Clone, PartialEq, Debug",
    deprecated = "warn"
)]
pub struct RevokeUserGrantsMutation;

#[derive(Clone, Debug, Builder)]
pub struct RevokeUserGrantsInput {
    #[builder(into)]
    pub organization_id: String,
    /// The OAuth client to revoke under: Rover's own, or a client-credential pair's client ID.
    #[builder(into)]
    pub client_id: String,
    #[builder(into)]
    pub user_id: String,
}
