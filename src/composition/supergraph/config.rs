#![allow(dead_code)]
use std::collections::BTreeMap;

use apollo_federation_types::config::SupergraphConfig;
use camino::Utf8PathBuf;

#[derive(thiserror::Error, Debug)]
#[error("error")]
pub struct LoadSupergraphConfigError;

pub struct ResolvedSupergraphConfig {
    // TODO: this will eventually contain values that have been validated for correctness, such as non-empty values on the subgraphs BTreeMap, and a resolved federation version
    inner: SupergraphConfig,
    path: Utf8PathBuf,
}

impl ResolvedSupergraphConfig {
    pub async fn load(
        path: &Utf8PathBuf,
    ) -> Result<ResolvedSupergraphConfig, LoadSupergraphConfigError> {
        let supergraph_config = SupergraphConfig::new(BTreeMap::default(), None);
        Ok(ResolvedSupergraphConfig {
            inner: supergraph_config,
            path: path.clone(),
        })
    }
    pub fn path(&self) -> &Utf8PathBuf {
        &self.path
    }
}
