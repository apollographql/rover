use std::{collections::HashMap, time::Duration};

#[cfg(test)]
pub(crate) type QueryResponseData =
    crate::operations::graph::introspect::service::graph_introspect_query::ResponseData;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphIntrospectInput {
    pub headers: HashMap<String, String>,
    pub endpoint: url::Url,
    pub should_retry: bool,
    pub retry_period: Duration,
    /// Use a pre-October-2021 introspection query that omits
    /// `includeDeprecated` on `args`/`inputFields` and
    /// `isDeprecated`/`deprecationReason` on `__InputValue`, for
    /// servers that don't implement those introspection additions.
    pub use_legacy_introspection_query: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphIntrospectResponse {
    pub schema_sdl: String,
}
