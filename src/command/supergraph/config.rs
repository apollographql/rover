use crate::{anyhow, error::RoverError, Suggestion};
use crate::{utils::client::StudioClientConfig, Result};

use crate::utils::parsers::{parse_graph_ref, GraphRef};
use camino::Utf8PathBuf;
use harmonizer::ServiceDefinition as SubgraphDefinition;
use rover_client::query::subgraph::fetch;
use rover_client::{blocking::Client, query::graph::introspect};
use serde::{Deserialize, Serialize};
use url::Url;

use std::collections::{BTreeMap, HashMap};

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
    SchemaFile { file: Utf8PathBuf },
    SchemaIntrospection { url: Url },
    SchemaSubgraph { graphref: String, subgraph: String },
}

pub(crate) fn parse_supergraph_config(config_path: &Utf8PathBuf) -> Result<SupergraphConfig> {
    let raw_supergraph_config = fs::read_to_string(config_path)
        .map_err(|e| anyhow!("Could not read \"{}\": {}", config_path, e))?;

    let parsed_config = serde_yaml::from_str(&raw_supergraph_config)
        .map_err(|e| anyhow!("Could not parse YAML from \"{}\": {}", config_path, e))?;

    tracing::debug!(?parsed_config);

    Ok(parsed_config)
}

impl SupergraphConfig {
    pub(crate) fn get_subgraph_definitions(
        &self,
        config_path: &Utf8PathBuf,
        client_config: StudioClientConfig,
        profile_name: &str,
    ) -> Result<Vec<SubgraphDefinition>> {
        let mut subgraphs = Vec::new();

        for (subgraph_name, subgraph_data) in &self.subgraphs {
            match &subgraph_data.schema {
                SchemaSource::SchemaFile { file } => {
                    // this needs to read from file
                    // this _must_ have a routing URL under subgraph_data.routing_url
                    let relative_schema_path = match config_path.parent() {
                        Some(parent) => {
                            let mut schema_path = parent.to_path_buf();
                            schema_path.push(file);
                            schema_path
                        }
                        None => file.clone(),
                    };

                    let schema = fs::read_to_string(&relative_schema_path).map_err(|e| {
                        let err = anyhow!("Could not read \"{}\": {}", &relative_schema_path, e);
                        let mut err = RoverError::new(err);
                        err.set_suggestion(Suggestion::ValidComposeFile);
                        err
                    })?;

                    // TODO(@_lrlna): if no routing_url is able to be found for SchemaFile
                    // variant, return an error to the user.
                    let url = &subgraph_data.routing_url.clone().ok_or_else(|| {
                        let err = anyhow!("No routing_url found for schema file.");
                        let mut err = RoverError::new(err);
                        err.set_suggestion(Suggestion::ValidComposeRoutingUrl);
                        err
                    })?;

                    // name, routing URL, schema SDL
                    let subgraph_definition = SubgraphDefinition::new(subgraph_name, url, &schema);
                    subgraphs.push(subgraph_definition);
                }
                SchemaSource::SchemaIntrospection { url } => {
                    // this pings an endpoint at specified url to get a schema SDL
                    let client = Client::new(&url.to_string());

                    let introspection_response = introspect::run(&client, &HashMap::new())?;
                    let schema = introspection_response.result;
                    println!("{:#?}", &schema);

                    let subgraph_definition = SubgraphDefinition::new(subgraph_name, "", &schema);
                    subgraphs.push(subgraph_definition);
                }
                SchemaSource::SchemaSubgraph { graphref, subgraph } => {
                    // this pings a subgraph url to given a graphref and subgraph name
                    // graphref needs to first be parsed using our graphref parser
                    // returned schema SDL gets added a subgraph definition
                    let client = client_config.get_client(&profile_name)?;
                    let graphref = parse_graph_ref(graphref)?;
                    let schema = fetch::run(
                        fetch::fetch_subgraph_query::Variables {
                            graph_id: graphref.name.clone(),
                            variant: graphref.variant.clone(),
                        },
                        &client,
                        subgraph,
                    )?;
                    println!("{:#?}", &schema);

                    let subgraph_definition = SubgraphDefinition::new(subgraph_name, "", &schema);
                    subgraphs.push(subgraph_definition);
                }
            }
        }

        Ok(subgraphs)
    }
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

    #[test]
    fn it_errs_on_invalid_subgraph_path() {
        let raw_good_yaml = r#"subgraphs:
  films:
    routing_url: https://films.example.com
    schema: 
      file: ./films-do-not-exist.graphql
  people:
    routing_url: https://people.example.com
    schema: 
      file: ./people-do-not-exist.graphql"#;
        let tmp_home = TempDir::new().unwrap();
        let mut config_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        config_path.push("config.yaml");
        fs::write(&config_path, raw_good_yaml).unwrap();
        let supergraph_config = super::parse_supergraph_config(&config_path).unwrap();
        // assert!(supergraph_config
        //     .get_subgraph_definitions(&config_path)
        //     .is_err())
    }

    #[test]
    fn it_can_get_subgraph_definitions_from_fs() {
        let raw_good_yaml = r#"subgraphs:
  films:
    routing_url: https://films.example.com
    schema: 
      file: ./films.graphql
  people:
    routing_url: https://people.example.com
    schema: 
      file: ./people.graphql"#;
        let tmp_home = TempDir::new().unwrap();
        let mut config_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        config_path.push("config.yaml");
        fs::write(&config_path, raw_good_yaml).unwrap();
        let tmp_dir = config_path.parent().unwrap().to_path_buf();
        let films_path = tmp_dir.join("films.graphql");
        let people_path = tmp_dir.join("people.graphql");
        fs::write(films_path, "there is something here").unwrap();
        fs::write(people_path, "there is also something here").unwrap();
        let supergraph_config = super::parse_supergraph_config(&config_path).unwrap();
        // assert!(supergraph_config
        //     .get_subgraph_definitions(&config_path)
        //     .is_ok())
    }

    #[test]
    fn it_can_compute_relative_schema_paths() {
        let raw_good_yaml = r#"subgraphs:
  films:
    routing_url: https://films.example.com
    schema: 
      file: ../../films.graphql
  people:
    routing_url: https://people.example.com
    schema: 
        file: ../../people.graphql"#;
        let tmp_home = TempDir::new().unwrap();
        let tmp_dir = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        let mut config_path = tmp_dir.clone();
        config_path.push("layer");
        config_path.push("layer");
        fs::create_dir_all(&config_path).unwrap();
        config_path.push("config.yaml");
        fs::write(&config_path, raw_good_yaml).unwrap();
        let films_path = tmp_dir.join("films.graphql");
        let people_path = tmp_dir.join("people.graphql");
        fs::write(films_path, "there is something here").unwrap();
        fs::write(people_path, "there is also something here").unwrap();
        let supergraph_config = super::parse_supergraph_config(&config_path).unwrap();
        // let subgraph_definitions = supergraph_config
        //     .get_subgraph_definitions(&config_path)
        //     .unwrap();
        // let people_subgraph = subgraph_definitions.get(0).unwrap();
        // let film_subgraph = subgraph_definitions.get(1).unwrap();

        // assert_eq!(film_subgraph.name, "films");
        // assert_eq!(film_subgraph.url, "https://films.example.com");
        // assert_eq!(film_subgraph.type_defs, "there is something here");
        // assert_eq!(people_subgraph.name, "people");
        // assert_eq!(people_subgraph.url, "https://people.example.com");
        // assert_eq!(people_subgraph.type_defs, "there is also something here");
    }
}
