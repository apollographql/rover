mod runner;
mod service;

use std::{fmt, time::Duration};

use graphql_client::GraphQLQuery;
use houston::CredentialOrigin;
pub use runner::run;
pub use service::{WhoAmI, WhoAmIError, WhoAmIRequest};

/// Conservative bound for a small identity query, in the absence of observed latency data.
pub const DEFAULT_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(GraphQLQuery)]
#[graphql(
    query_path = "src/operations/config/who_am_i/who_am_i_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// Platform API identity lookup.
pub struct ConfigWhoAmIQuery;

impl fmt::Debug for config_who_am_i_query::Variables {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Variables").finish()
    }
}

impl PartialEq for config_who_am_i_query::Variables {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

pub(crate) type QueryVariables = config_who_am_i_query::Variables;

#[derive(Debug, Eq, PartialEq)]
pub struct RegistryIdentity {
    pub id: String,
    pub graph_title: Option<String>,
    pub key_actor_type: Actor,
    pub credential_origin: CredentialOrigin,
}

// Variant names deliberately mirror the platform API's `ActorType` enum values.
#[allow(non_camel_case_types)]
#[derive(Debug, Eq, PartialEq)]
pub enum Actor {
    GRAPH,
    USER,
    SERVICE_ACCOUNT,
    OTHER,
}

impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Actor::GRAPH => write!(f, "Graph"),
            Actor::USER => write!(f, "User"),
            Actor::SERVICE_ACCOUNT => write!(f, "Service Account"),
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
