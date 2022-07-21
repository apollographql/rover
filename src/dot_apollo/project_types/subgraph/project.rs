use std::collections::BTreeMap;

use super::{SchemaSource, SubgraphConfig};
use buildstructor::buildstructor;
use reqwest::Url;
use saucer::{anyhow, Result, Utf8Path};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MultiSubgraphConfig {
    supergraph: ExtendSupergraphConfig,

    // Store config in a BTreeMap, as HashMap is non-deterministic.
    subgraphs: BTreeMap<String, SubgraphConfig>,
}

#[buildstructor]
impl MultiSubgraphConfig {
    pub(crate) fn new() -> Self {
        Self {
            supergraph: ExtendSupergraphConfig { graph_id: None },
            subgraphs: BTreeMap::new(),
        }
    }

    #[builder(entry = "supergraph", exit = "extend")]
    pub(crate) fn extend_supergraph(&mut self, graph_id: String) -> Result<()> {
        if let Some(graph_id) = &self.supergraph.graph_id {
            Err(anyhow!(
                "supergraph with graph ID {} already exists",
                graph_id
            ))
        } else {
            self.supergraph = ExtendSupergraphConfig {
                graph_id: Some(graph_id),
            };
            Ok(())
        }
    }

    #[builder(entry = "subgraph", exit = "add")]
    pub(crate) fn add_subgraph(&mut self, name: String, config: SubgraphConfig) -> Result<()> {
        if self.subgraphs.get(&name).is_some() {
            Err(anyhow!(
                "could not extend subgraph config because {} already exists",
                &name
            ))
        } else {
            self.subgraphs.insert(name, config);
            Ok(())
        }
    }

    // #[builder(entry = "schema", exit = "extend")]
    // pub(crate) fn add_schema_subgraph<F>(
    //     &mut self,
    //     name: String,
    //     file: F,
    //     local_endpoint: Url,
    //     remote_endpoint: Option<Url>,
    // ) -> Result<()>
    // where
    //     F: AsRef<Utf8Path>,
    // {
    //     let file = file.as_ref().to_path_buf();
    //     let subgraph_config = SubgraphConfig {
    //         schema: SchemaSource::File { file },
    //         local_endpoint,
    //         remote_endpoint,
    //     };
    //     if self.subgraphs.get(&name).is_some() {
    //         Err(anyhow!(
    //             "could not extend subgraph config because {} already exists",
    //             &name
    //         ))
    //     } else {
    //         self.subgraphs.insert(name, subgraph_config);
    //         Ok(())
    //     }
    // }

    // #[builder(entry = "url", exit = "extend")]
    // pub(crate) fn add_url_subgraph(
    //     &mut self,
    //     name: String,
    //     subgraph_url: Url,
    //     local_endpoint: Url,
    //     remote_endpoint: Option<Url>,
    // ) -> Result<()> {
    //     let subgraph_config = SubgraphConfig {
    //         schema: SchemaSource::SubgraphIntrospection { subgraph_url },
    //         local_endpoint,
    //         remote_endpoint,
    //     };
    //     if self.subgraphs.get(&name).is_some() {
    //         Err(anyhow!(
    //             "could not extend subgraph config because {} already exists",
    //             &name
    //         ))
    //     } else {
    //         self.subgraphs.insert(name, subgraph_config);
    //         Ok(())
    //     }
    // }

    // #[builder(entry = "studio_subgraph", exit = "extend")]
    // pub(crate) fn add_studio_subgraph(
    //     &mut self,
    //     name: String,
    //     graphref: String,
    //     subgraph: String,
    //     local_endpoint: Url,
    //     remote_endpoint: Option<Url>,
    // ) -> Result<()> {
    //     let subgraph_config = SubgraphConfig {
    //         schema: SchemaSource::Subgraph { graphref, subgraph },
    //         local_endpoint,
    //         remote_endpoint,
    //     };
    //     if self.subgraphs.get(&name).is_some() {
    //         Err(anyhow!(
    //             "could not extend subgraph config because {} already exists",
    //             &name
    //         ))
    //     } else {
    //         self.subgraphs.insert(name, subgraph_config);
    //         Ok(())
    //     }
    // }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExtendSupergraphConfig {
    graph_id: Option<String>,
}
