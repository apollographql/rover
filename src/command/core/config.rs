use crate::{anyhow, Result};

use camino::Utf8PathBuf;
use harmonizer::ServiceDefinition as SubgraphDefinition;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CoreConfig {
    pub(crate) subgraphs: HashMap<String, Subgraph>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Subgraph {
    pub(crate) routing_url: String,
    pub(crate) schema_path: Utf8PathBuf,
}

pub(crate) fn parse_core_config(config_path: &Utf8PathBuf) -> Result<CoreConfig> {
    let raw_core_config = fs::read_to_string(config_path)
        .map_err(|e| anyhow!("Could not read \"{}\": {}", config_path, e))?;

    let parsed_config = serde_yaml::from_str(&raw_core_config)
        .map_err(|e| anyhow!("Could not parse YAML from \"{}\": {}", config_path, e))?;

    tracing::debug!(?parsed_config);

    Ok(parsed_config)
}

impl CoreConfig {
    pub(crate) fn get_subgraph_definitions(
        &self,
        config_path: &Utf8PathBuf,
    ) -> Result<Vec<SubgraphDefinition>> {
        let mut subgraphs = Vec::new();

        for (subgraph_name, subgraph_data) in &self.subgraphs {
            // compute the path to the schema relative to the config file itself, not the working directory.
            let relative_schema_path = if let Some(parent) = config_path.parent() {
                let mut schema_path = parent.to_path_buf();
                schema_path.push(&subgraph_data.schema_path);
                schema_path
            } else {
                subgraph_data.schema_path.clone()
            };

            let schema = fs::read_to_string(&relative_schema_path)
                .map_err(|e| anyhow!("Could not read \"{}\": {}", &relative_schema_path, e))?;

            let subgraph_definition =
                SubgraphDefinition::new(subgraph_name, &subgraph_data.routing_url, &schema);

            subgraphs.push(subgraph_definition);
        }

        Ok(subgraphs)
    }
}
