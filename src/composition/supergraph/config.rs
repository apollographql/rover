#![allow(dead_code)]
use std::collections::BTreeMap;

use apollo_federation_types::{build::SubgraphDefinition, config::FederationVersion};
use camino::Utf8PathBuf;

#[derive(thiserror::Error, Debug)]
#[error("error")]
pub struct LoadSupergraphConfigError;

pub struct ResolvedSupergraphConfig {
    subgraphs: BTreeMap<String, SubgraphDefinition>,
    path: Utf8PathBuf,
    federation_version: FederationVersion,
}

impl ResolvedSupergraphConfig {
    pub async fn load(
        path: &Utf8PathBuf,
    ) -> Result<ResolvedSupergraphConfig, LoadSupergraphConfigError> {
        Ok(ResolvedSupergraphConfig {
            subgraphs: BTreeMap::new(),
            federation_version: FederationVersion::LatestFedTwo,
            path: path.clone(),
        })
    }
    pub fn path(&self) -> &Utf8PathBuf {
        &self.path
    }

    pub fn federation_version(&self) -> FederationVersion {
        self.federation_version.clone()
    }
}
