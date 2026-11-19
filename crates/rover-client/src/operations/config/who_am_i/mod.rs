mod runner;
mod service;
mod types;

use std::{fmt, time::Duration};

use graphql_client::GraphQLQuery;
pub use runner::run;
pub use service::{WhoAmI, WhoAmIError, WhoAmIRequest};
pub use types::{Actor, ConfigWhoAmIInput, RegistryIdentity};

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
