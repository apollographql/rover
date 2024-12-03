use std::fmt::{Display, Formatter, Result};

use super::service::config_who_am_i_query;

use houston::CredentialOrigin;

pub(crate) type QueryVariables = config_who_am_i_query::Variables;

#[derive(Debug, Eq, PartialEq)]
pub struct RegistryIdentity {
    pub id: String,
    pub graph_title: Option<String>,
    pub key_actor_type: Actor,
    pub credential_origin: CredentialOrigin,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Actor {
    GRAPH,
    USER,
    OTHER,
}

impl Display for Actor {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            Actor::GRAPH => write!(f, "Graph"),
            Actor::USER => write!(f, "User"),
            Actor::OTHER => write!(f, "Other"),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ConfigWhoAmIInput {}

impl From<ConfigWhoAmIInput> for QueryVariables {
    fn from(_input: ConfigWhoAmIInput) -> Self {
        Self {}
    }
}
