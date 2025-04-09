use crate::composition::supergraph::config::lazy::LazilyResolvedSubgraph;
use crate::RoverResult;
use apollo_federation_types::config::{
    FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
};
use camino::Utf8PathBuf;
use itertools::Itertools;
use rover_std::infoln;
use rover_std::prompt::prompt_confirm_default_yes;
use std::collections::BTreeMap;
use std::fs::File;
use std::io;
use std::path::PathBuf;

pub struct TemplateOperations;

impl TemplateOperations {
    pub fn prompt_creation(artifacts: Vec<Utf8PathBuf>) -> io::Result<bool> {
        println!("The following files will be created:");
        let mut artifacts_sorted = artifacts;
        artifacts_sorted.sort();

        Self::print_grouped_files(artifacts_sorted);

        println!();
        prompt_confirm_default_yes("Proceed with creation?")
    }

    pub fn print_grouped_files(artifacts: Vec<Utf8PathBuf>) {
        for (_, files) in &artifacts
            .into_iter()
            .chunk_by(|artifact| artifact.parent().map(|p| p.to_owned()))
        {
            for file in files {
                if file.file_name().is_some() {
                    infoln!("{}", file);
                }
            }
        }
    }

    pub fn generate_supergraph(
        output_dir: Utf8PathBuf,
        subgraph: LazilyResolvedSubgraph,
    ) -> RoverResult<()> {
        let mut subgraph_map = BTreeMap::new();
        subgraph_map.insert(subgraph.name().clone(), SubgraphConfig::from(subgraph));

        let supergraph = SupergraphConfig::new(subgraph_map, Some(FederationVersion::LatestFedTwo));

        let file_writer = File::create(output_dir.join("supergraph.yaml"))?;
        serde_yaml::to_writer(file_writer, &supergraph)?;
        Ok(())
    }

    pub fn create_subgraph_config(
        url: String,
        schema_file_path: String,
        subgraph_name: String,
    ) -> RoverResult<LazilyResolvedSubgraph> {
        let schema_source = SchemaSource::File {
            file: PathBuf::from(schema_file_path),
        };

        let subgraph = LazilyResolvedSubgraph::builder()
            .routing_url(url)
            .schema(schema_source)
            .name(subgraph_name)
            .build();

        Ok(subgraph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, fs};

    #[test]
    pub fn test_writing_supergraph_yaml_new() {
        let subgraph = TemplateOperations::create_subgraph_config(
            "http://localhost:4000".to_string(),
            "test.graphql".to_string(),
            "test".to_string(),
        )
        .unwrap();

        let temp_dir = env::temp_dir();
        let output_dir = Utf8PathBuf::from_path_buf(temp_dir).unwrap();
        let expected_path = output_dir.join("supergraph.yaml");

        TemplateOperations::generate_supergraph(output_dir, subgraph).unwrap();

        let supergraph_yaml_content = std::fs::read_to_string(expected_path).unwrap();
        assert_eq!(
            supergraph_yaml_content,
            "subgraphs:\n  test:\n    routing_url: http://localhost:4000\n    schema:\n      file: test.graphql\nfederation_version: '2'\n"
        );
    }

    #[test]
    pub fn test_writing_supergraph_yaml_replace_existing() {
        let temp_dir = env::temp_dir();
        let output_dir = Utf8PathBuf::from_path_buf(temp_dir).unwrap();
        let original_path = output_dir.join("supergraph.yaml");
        let expected_path = original_path.clone();

        let original = "subgraphs:\n  should_be_replaced:\n    routing_url: http://localhost:4000\n    schema:\n      file: test.graphql\nfederation_version: '2'\n";
        fs::write(original_path, original).unwrap();

        let subgraph = TemplateOperations::create_subgraph_config(
            "http://localhost:4000".to_string(),
            "test.graphql".to_string(),
            "test".to_string(),
        )
        .unwrap();

        TemplateOperations::generate_supergraph(output_dir, subgraph).unwrap();

        let supergraph_yaml_content = std::fs::read_to_string(expected_path).unwrap();
        assert_eq!(
            supergraph_yaml_content,
            "subgraphs:\n  test:\n    routing_url: http://localhost:4000\n    schema:\n      file: test.graphql\nfederation_version: '2'\n"
        );
    }
}
