use crate::operations::init::key::runner::init_new_key_mutation;
<<<<<<< HEAD
=======
use crate::shared::GraphRef;
>>>>>>> 2026a3ce (Adding Graph key creation)

type MutationVariables = init_new_key_mutation::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct InitNewKeyInput {
<<<<<<< HEAD
    pub graph_id: String,
=======
    pub graph_ref: GraphRef,
>>>>>>> 2026a3ce (Adding Graph key creation)
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
            graph_id: input.graph_id,
            key_name: input.key_name,
            role: match input.role {
                UserPermission::GraphAdmin => init_new_key_mutation::UserPermission::GRAPH_ADMIN,
            },
        }
    }
}
