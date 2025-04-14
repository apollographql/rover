use super::service::init_memberships_query;

use houston::CredentialOrigin;
use serde::Serialize;

pub(crate) type QueryVariables = init_memberships_query::Variables;

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct Organization {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct InitMembershipsResponse {
    pub id: String,
    pub memberships: Vec<Organization>,
    #[serde(skip_serializing)]
    pub credential_origin: CredentialOrigin,
}

#[derive(Debug, Eq, PartialEq)]
pub struct InitMembershipsInput {}

impl From<InitMembershipsInput> for QueryVariables {
    fn from(_input: InitMembershipsInput) -> Self {
        Self {}
    }
}
