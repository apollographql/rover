use crate::operations::init::key::runner::init_new_key_mutation;
use crate::shared::GraphRef;

type MutationVariables = init_new_key_mutation::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InitNewKeyInput {
    pub graph_ref: GraphRef,
    pub key_name: String,
    pub role: UserPermission,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InitNewKeyResponse {
    pub token: String,
    pub id: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum UserPermission {
    GraphAdmin,
}

impl From<InitNewKeyInput> for MutationVariables {
    fn from(input: InitNewKeyInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
            key_name: input.key_name,
            role: match input.role {
                UserPermission::GraphAdmin => init_new_key_mutation::UserPermission::GRAPH_ADMIN,
            },
        }
    }
}
