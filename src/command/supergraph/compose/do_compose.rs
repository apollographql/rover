use crate::command::supergraph::config::{self, SchemaSource, SupergraphConfig};
use crate::utils::{client::StudioClientConfig, parsers::parse_graph_ref};
use crate::{anyhow, command::RoverStdout, error::RoverError, Result, Suggestion};

use ansi_term::Colour::Red;
use camino::Utf8PathBuf;

use rover_client::{
    blocking::Client,
    query::subgraph::{fetch, introspect},
};
use serde::Serialize;
use std::{collections::HashMap, fs};
use structopt::StructOpt;

use harmonizer::ServiceDefinition as SubgraphDefinition;

#[derive(Debug, Serialize, StructOpt)]
pub struct Compose {
    /// The relative path to the supergraph configuration file.
    #[structopt(long = "config")]
    #[serde(skip_serializing)]
    config_path: Utf8PathBuf,

    /// Name of configuration profile to use
    #[structopt(long = "profile", default_value = "default")]
    #[serde(skip_serializing)]
    profile_name: String,
}

impl Compose {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverStdout> {
        let supergraph_config = config::parse_supergraph_config(&self.config_path)?;
        let subgraph_definitions = get_subgraph_definitions(
            supergraph_config,
            &self.config_path,
            client_config,
            &self.profile_name,
        )?;

        match harmonizer::harmonize(subgraph_definitions) {
            Ok(core_schema) => Ok(RoverStdout::CoreSchema(core_schema)),
            Err(composition_errors) => {
                let num_failures = composition_errors.len();
                for composition_error in composition_errors {
                    eprintln!("{} {}", Red.bold().paint("error:"), &composition_error)
                }
                match num_failures {
                    0 => unreachable!("Composition somehow failed with no composition errors."),
                    1 => Err(
                        anyhow!("Encountered 1 composition error while composing the graph.")
                            .into(),
                    ),
                    _ => Err(anyhow!(
                        "Encountered {} composition errors while composing the graph.",
                        num_failures
                    )
                    .into()),
                }
            }
        }
    }
}

pub(crate) fn get_subgraph_definitions(
    supergraph_config: SupergraphConfig,
    config_path: &Utf8PathBuf,
    client_config: StudioClientConfig,
    profile_name: &str,
) -> Result<Vec<SubgraphDefinition>> {
    let mut subgraphs = Vec::new();

    for (subgraph_name, subgraph_data) in &supergraph_config.subgraphs {
        match &subgraph_data.schema {
            SchemaSource::File { file } => {
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

                let url = &subgraph_data.routing_url.clone().ok_or_else(|| {
                    let err = anyhow!("No routing_url found for schema file.");
                    let mut err = RoverError::new(err);
                    err.set_suggestion(Suggestion::ValidComposeRoutingUrl);
                    err
                })?;

                let subgraph_definition = SubgraphDefinition::new(subgraph_name, url, &schema);
                subgraphs.push(subgraph_definition);
            }
            SchemaSource::SubgraphIntrospection { subgraph_url } => {
                // given a federated introspection URL, use subgraph introspect to
                // obtain SDL and add it to subgraph_definition.
                let client = Client::new(&subgraph_url.to_string());

                let introspection_response = introspect::run(&client, &HashMap::new())?;
                let schema = introspection_response.result;

                // We don't require a routing_url for this variant of a schema,
                // if none are provided, just use an empty string.
                let url = &subgraph_data
                    .routing_url
                    .clone()
                    .unwrap_or_else(|| subgraph_url.to_string());

                let subgraph_definition = SubgraphDefinition::new(subgraph_name, url, &schema);
                subgraphs.push(subgraph_definition);
            }
            SchemaSource::Subgraph { graphref, subgraph } => {
                // given a graphref and subgraph, run subgraph fetch to
                // obtain SDL and add it to subgraph_definition.
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

                // We don't require a routing_url for this variant of a schema,
                // if none are provided, just use an empty string.
                //
                // TODO: this should eventually get the url from the registry
                // and use that when no routing_url is provided.
                let url = &subgraph_data.routing_url.clone().unwrap_or_default();

                let subgraph_definition = SubgraphDefinition::new(subgraph_name, url, &schema);
                subgraphs.push(subgraph_definition);
            }
        }
    }

    Ok(subgraphs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use houston as houston_config;
    use houston_config::Config;
    use std::convert::TryFrom;

    fn get_studio_config() -> StudioClientConfig {
        let tmp_home = TempDir::new().unwrap();
        let tmp_path = Utf8PathBuf::try_from(tmp_home.path().to_path_buf()).unwrap();
        StudioClientConfig::new(None, Config::new(Some(&tmp_path), None).unwrap())
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
        let supergraph_config = config::parse_supergraph_config(&config_path).unwrap();
        assert!(get_subgraph_definitions(
            supergraph_config,
            &config_path,
            get_studio_config(),
            "profile"
        )
        .is_err())
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
        let supergraph_config = config::parse_supergraph_config(&config_path).unwrap();
        assert!(get_subgraph_definitions(
            supergraph_config,
            &config_path,
            get_studio_config(),
            "profile"
        )
        .is_ok())
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
        let supergraph_config = config::parse_supergraph_config(&config_path).unwrap();
        let subgraph_definitions = get_subgraph_definitions(
            supergraph_config,
            &config_path,
            get_studio_config(),
            "profile",
        )
        .unwrap();
        let film_subgraph = subgraph_definitions.get(0).unwrap();
        let people_subgraph = subgraph_definitions.get(1).unwrap();

        assert_eq!(film_subgraph.name, "films");
        assert_eq!(film_subgraph.url, "https://films.example.com");
        assert_eq!(film_subgraph.type_defs, "there is something here");
        assert_eq!(people_subgraph.name, "people");
        assert_eq!(people_subgraph.url, "https://people.example.com");
        assert_eq!(people_subgraph.type_defs, "there is also something here");
    }
}
