use bon::Builder;
use graphql_client::GraphQLQuery;

pub mod service;

pub use service::{OrgMembership, ORG_MEMBERSHIP_ATTEMPT_TIMEOUT};

/// Reads an organization's members, to tell whether one user is still among them - how
/// `rover auth grants revoke`'s per-user sweep warns about a user who has already left the
/// organization (spec FR60, `specs/rover-431-identity-grant-management`).
#[derive(GraphQLQuery, Debug)]
#[graphql(
    query_path = "src/operations/auth/org_membership/org_membership_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    variables_derives = "Clone, PartialEq, Debug",
    deprecated = "warn"
)]
pub struct OrgMembershipQuery;

#[derive(Clone, Debug, Builder)]
pub struct OrgMembershipInput {
    #[builder(into)]
    pub organization_id: String,
    #[builder(into)]
    pub user_id: String,
}
