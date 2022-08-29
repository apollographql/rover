use crate::{command::dev::socket::SubgraphUrl, Result};
use apollo_federation_types::build::SubgraphDefinition;

use std::collections::HashSet;

pub struct Context {
    subgraph: SubgraphDefinition,
    router: Option<RouterDefinition>,
}

impl Context {
    pub fn new() -> Context {}
}

pub struct RouterDefinition {
    endpoint: SubgraphUrl,
}

impl RouterDefinition {
    pub fn endpoints(&self) -> Result<HashSet<SubgraphUrl>> {
        Ok(self
            .endpoint
            .socket_addrs(|| None)
            .map(|sas| {
                sas.iter()
                    .filter_map(|s| {
                        format!("http://{}:{}", s.ip(), s.port())
                            .parse::<SubgraphUrl>()
                            .ok()
                    })
                    .collect()
            })
            .unwrap_or_else(|_| HashSet::new()))
    }
}
