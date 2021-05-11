use crate::{anyhow, Result};

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use url::Url;

use std::collections::BTreeMap;

use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SupergraphConfig {
    // Store config in a BTreeMap, as HashMap is non-deterministic.
    pub(crate) subgraphs: BTreeMap<String, Subgraph>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Subgraph {
    pub(crate) routing_url: Option<String>,
    pub(crate) schema: SchemaSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum SchemaSource {
    File { file: Utf8PathBuf },
    SubgraphIntrospection { subgraph_url: Url },
    Subgraph { graphref: String, subgraph: String },
}

pub(crate) fn parse_supergraph_config(config_path: &Utf8PathBuf) -> Result<SupergraphConfig> {
    let raw_supergraph_config = fs::read_to_string(config_path)
        .map_err(|e| anyhow!("Could not read \"{}\": {}", config_path, e))?;

    let parsed_config = serde_yaml::from_str(&raw_supergraph_config)
        .map_err(|e| anyhow!("Could not parse YAML from \"{}\": {}", config_path, e))?;

    tracing::debug!(?parsed_config);

    Ok(parsed_config)
}
#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;
    use std::convert::TryFrom;
    use std::fs;

    #[test]
    fn it_can_parse_valid_config() {
        let raw_good_yaml = r#"subgraphs:
  films:
    routing_url: https://films.example.com
    schema: 
      file: ./good-films.graphql
  people:
    routing_url: https://people.example.com
    schema: 
      file: ./good-people.graphql
"#;
        let tmp_home = TempDir::new().unwrap();
        let mut config_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        config_path.push("config.yaml");
        fs::write(&config_path, raw_good_yaml).unwrap();

        let supergraph_config = super::parse_supergraph_config(&config_path);
        if let Err(e) = supergraph_config {
            panic!("{}", e)
        }
    }
    #[test]
    fn it_can_parse_valid_config_with_introspection() {
        let raw_good_yaml = r#"subgraphs:
  films:
    routing_url: https://films.example.com
    schema:
      file: ./films.graphql
  people:
    schema: 
      url: https://people.example.com
  reviews:
    schema:
      graphref: mygraph@current
      subgraph: reviews    
"#;
        let tmp_home = TempDir::new().unwrap();
        let mut config_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        config_path.push("config.yaml");
        fs::write(&config_path, raw_good_yaml).unwrap();

        let supergraph_config = super::parse_supergraph_config(&config_path);
        if let Err(e) = supergraph_config {
            panic!("{}", e)
        }
    }

    #[test]
    fn it_errors_on_invalid_config() {
        let raw_bad_yaml = r#"subgraphs:
  films:
    routing_______url: https://films.example.com
    schemaaaa: 
        file:: ./good-films.graphql
  people:
    routing____url: https://people.example.com
    schema_____file: ./good-people.graphql"#;
        let tmp_home = TempDir::new().unwrap();
        let mut config_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        config_path.push("config.yaml");
        fs::write(&config_path, raw_bad_yaml).unwrap();
        assert!(super::parse_supergraph_config(&config_path).is_err())
    }
}
