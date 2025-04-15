use crate::composition::supergraph::config::lazy::LazilyResolvedSubgraph;
use crate::{RoverError, RoverResult};
use anyhow::format_err;
use apollo_federation_types::config::{
    FederationVersion, SchemaSource, SubgraphConfig, SupergraphConfig,
};
use camino::Utf8PathBuf;
use itertools::Itertools;
use rover_std::infoln;
use rover_std::prompt::prompt_confirm_default_yes;
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::{fs, io};
use rover_std::Style;

pub struct TemplateOperations;

impl TemplateOperations {
    pub fn prompt_creation(artifacts: Vec<Utf8PathBuf>) -> io::Result<bool> {
        println!("The following files will be created:");
        let mut artifacts_sorted = artifacts;
        artifacts_sorted.sort();

        Self::print_grouped_files(artifacts_sorted);

        println!();
        prompt_confirm_default_yes(&Style::Prompt.paint("? Proceed with creation?"))
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
}

pub struct SupergraphBuilder {
    directory: Utf8PathBuf,
    max_depth: usize,
}

impl SupergraphBuilder {
    pub fn new(directory: Utf8PathBuf, max_depth: usize) -> Self {
        Self {
            directory,
            max_depth,
        }
    }

    /*
       In this fn we collect all graphql schemas found in the directory,
       also try to disambiguate names in case that they end up being duplicate
       by counting all resolved names to make sure there are no duplicates,
       depending on the structure of the project, there is a chance that if we only use
       the parent for naming, there might be duplicates. for example
       /root
         /products
           /model
             /schema.graphql
         /services
           /model
             /schema.graphql
    */
    pub fn generate_subgraphs(&self) -> RoverResult<BTreeMap<String, SubgraphConfig>> {
        let mut subgraphs = BTreeMap::new();

        // Collect all graphql schemas
        let graphql_files = self.find_graphql_files(&self.directory, self.max_depth)?;

        if graphql_files.is_empty() {
            return Err(RoverError::from(format_err!(
                "No graphql files found in the directory"
            )));
        }

        for file_path in graphql_files {
            let mut name = self.determine_subgraph_name(&file_path)?;

            // Check if the name already exists amd disambiguate if so
            if subgraphs.contains_key(&name) {
                name = self.disambiguate_name(&file_path, &name)?;
            }

            let file = self.directory.as_std_path().join(file_path);
            println!("File: {}", file.display());
            let subgraph = LazilyResolvedSubgraph::builder()
                .name(name.clone())
                .routing_url("http://ignore".to_string()) // Hardcoded URL
                .schema(SchemaSource::File {
                    file: file.clone(),
                })
                .build();

            subgraphs.insert(name, SubgraphConfig::from(subgraph));
        }

        Ok(subgraphs)
    }

    fn find_graphql_files(&self, dir: &Utf8PathBuf, max_depth: usize) -> RoverResult<Vec<PathBuf>> {
        let mut graphql_files = Vec::new();
        self.visit_dirs(dir.as_std_path(), 0, max_depth, &mut graphql_files)?;
        Ok(graphql_files)
    }

    fn visit_dirs(
        &self,
        dir: &Path,
        current_depth: usize,
        max_depth: usize,
        result: &mut Vec<PathBuf>,
    ) -> RoverResult<()> {
        if current_depth > max_depth {
            return Ok(());
        }

        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    self.visit_dirs(&path, current_depth + 1, max_depth, result)?;
                } else if self.is_graphql_file(&path) {
                    let path =
                        Self::strip_base_prefix(path.as_path(), self.directory.as_std_path());
                    result.push(path);
                }
            }
        }

        Ok(())
    }

    fn strip_base_prefix(path: &Path, base_prefix: &Path) -> PathBuf {
        path.strip_prefix(base_prefix).unwrap().to_owned()
    }

    fn is_graphql_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            ext == "graphql" || ext == "graphqls"
        } else {
            false
        }
    }

    // If the file is named "schema", use parent directory name
    fn determine_subgraph_name(&self, file_path: &Path) -> RoverResult<String> {
        let file_stem = file_path.file_stem().unwrap().to_string_lossy();
        if file_stem == "schema" {
            let parent = file_path.parent().unwrap();
            let parent_name = parent.file_name().unwrap().to_string_lossy();
            Ok(parent_name.to_string())
        } else {
            Ok(file_stem.to_string())
        }
    }

    // Use parent's parent directory name for disambiguation
    fn disambiguate_name(&self, file_path: &Path, base_name: &str) -> RoverResult<String> {
        let parent_parent = file_path.parent().and_then(|p| p.parent()).unwrap();
        let parent_parent_name = parent_parent.file_name().unwrap().to_string_lossy();
        Ok(format!("{}_{}", parent_parent_name, base_name))
    }

    pub fn build_supergraph(&self) -> RoverResult<SupergraphConfig> {
        let subgraphs = self.generate_subgraphs()?;
        Ok(SupergraphConfig::new(
            subgraphs,
            Some(FederationVersion::LatestFedTwo),
        ))
    }

    pub fn build_and_write(&self) -> RoverResult<()> {
        let supergraph = self.build_supergraph()?;
        let output_path = self.directory.join("supergraph.yaml");
        let mut file = File::create(output_path)?;
        serde_yaml::to_writer(&mut file, &supergraph)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    // Helper function to create a GraphQL file in a temp directory
    fn create_graphql_file(base_dir: &Path, rel_path: &str, content: &str) -> io::Result<()> {
        let file_path = base_dir.join(rel_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(file_path)?;
        file.write_all(content.as_bytes())?;

        Ok(())
    }

    #[test]
    fn test_root_level_files() -> io::Result<()> {
        let temp_dir = tempdir()?;
        let path = Utf8PathBuf::from_path_buf(temp_dir.path().to_owned()).unwrap();

        // Create a single root level GraphQL file
        create_graphql_file(
            temp_dir.path(),
            "services.graphql",
            "type Service { id: ID! }",
        )?;

        let supergraph_builder = SupergraphBuilder::new(path, 5);

        supergraph_builder.build_and_write().unwrap();
        let expected = supergraph_builder.build_supergraph().unwrap();

        let actual_file = File::open(temp_dir.path().join("supergraph.yaml"))?;
        let actual: SupergraphConfig = serde_yaml::from_reader(actual_file).unwrap();
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn test_deep_nested_file() -> io::Result<()> {
        let temp_dir = tempdir()?;
        let path = Utf8PathBuf::from_path_buf(temp_dir.path().to_owned()).unwrap();

        create_graphql_file(
            temp_dir.path(),
            "products/schema.graphql",
            "type Product { id: ID! }",
        )?;

        let supergraph_builder = SupergraphBuilder::new(path, 5);

        supergraph_builder.build_and_write().unwrap();
        let expected = supergraph_builder.build_supergraph().unwrap();

        let actual_file = File::open(temp_dir.path().join("supergraph.yaml"))?;
        let actual: SupergraphConfig = serde_yaml::from_reader(actual_file).unwrap();
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn test_multiple_deep_nested_file() -> io::Result<()> {
        let temp_dir = tempdir()?;
        let path = Utf8PathBuf::from_path_buf(temp_dir.path().to_owned()).unwrap();

        create_graphql_file(
            temp_dir.path(),
            "products/schema.graphql",
            "type Product { id: ID! }",
        )?;

        create_graphql_file(
            temp_dir.path(),
            "services/schema.graphql",
            "type Service { id: ID! }",
        )?;

        let supergraph_builder = SupergraphBuilder::new(path, 5);

        supergraph_builder.build_and_write().unwrap();
        let expected = supergraph_builder.build_supergraph().unwrap();

        let actual_file = File::open(temp_dir.path().join("supergraph.yaml"))?;
        let actual: SupergraphConfig = serde_yaml::from_reader(actual_file).unwrap();
        assert_eq!(actual, expected);

        Ok(())
    }

    #[test]
    fn test_disambiguation() -> io::Result<()> {
        let temp_dir = tempdir()?;
        let path = Utf8PathBuf::from_path_buf(temp_dir.path().to_owned()).unwrap();

        create_graphql_file(
            temp_dir.path(),
            "products/model/schema.graphql",
            "type Product { id: ID! }",
        )?;

        create_graphql_file(
            temp_dir.path(),
            "services/model/schema.graphql",
            "type Service { id: ID! }",
        )?;

        create_graphql_file(
            temp_dir.path(),
            "billing/billing.graphql",
            "type Billing { id: ID! }",
        )?;

        let supergraph_builder = SupergraphBuilder::new(path, 5);

        supergraph_builder.build_and_write().unwrap();
        let expected = supergraph_builder.build_supergraph().unwrap();

        let actual_file = File::open(temp_dir.path().join("supergraph.yaml"))?;
        let actual: SupergraphConfig = serde_yaml::from_reader(actual_file).unwrap();
        assert_eq!(actual, expected);

        Ok(())
    }
}
