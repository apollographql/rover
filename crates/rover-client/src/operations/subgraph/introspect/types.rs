use std::{collections::HashMap, time::Duration};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphIntrospectInput {
    pub headers: HashMap<String, String>,
    pub endpoint: url::Url,
    pub should_retry: bool,
    pub retry_period: Duration,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SubgraphIntrospectResponse {
    pub result: String,
}
